//! `ExecutionCompiler` — `AthenaRequest` + Session 快照 → [`ExecutionModule`]。
//!
//! 引导 lowering：原子项、类型化 Boolean 常量、`ControlPlan::Branch` /
//! `Sequence`，以及经 `WriteBinding` 的有副作用 `SessionCommand::Define`。
//! 不桥接到已删除的栈式解释器。

use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode};
use athena_types::{BindingEvaluationPolicy, BindingKind, Diagnostic, DiagnosticCode, Result, TermId};

use crate::{
    api::request::{AthenaRequest, ControlPlan, SessionCommand},
    execution::ir::{
        BasicBlock, BlockEdge, BlockId, CapturedRoot, CapturedRootId, ConstantId, ConstantValue, EffectEdge, EffectKind, EffectToken,
        ExecutionModule, ExecutionValueType, ModuleFingerprint, Operation, OperationKind, ProviderCallDescriptor, ProviderCallId, Region,
        RegionId, SsaValueId, Terminator, verify_module,
    },
    runtime::session::Session,
};

/// 将一次请求编译为已校验的 [`ExecutionModule`]。
#[derive(Debug, Default)]
pub struct ExecutionCompiler {}

mod builder;
mod control;
mod define;
mod dump;
mod helpers;
mod stages;

pub use dump::{
    CompileObservation, CfgSsaStageView, PlanStageView, RequestStageView, SemanticStageView, dump_cfg_ssa, dump_plan,
    dump_request, dump_semantic, observe_compile, verify_observation,
};
pub use stages::{
    CfgSsaProgram, CompileStageKind, PlanIntent, PlanProgram, RequestProgram, SemanticOpSummary, SemanticProgram,
    StageFingerprint, StagedCompile, canonicalize_request, materialize_cfg_ssa, materialize_semantic, plan_from_request,
};

use builder::ModuleBuilder;
use helpers::{collect_compare_chain_args, flatten_compare_chain_args};

impl ExecutionCompiler {
    /// 创建编译器实例。
    pub fn new() -> Self {
        Self {}
    }

    /// 对照 Session 快照将请求 lowering 为 `ExecutionIR`。
    ///
    /// 先产出 [`RequestProgram`] / [`PlanProgram`]，再走 fused CFG lowering。
    pub fn compile(&self, session: &mut Session, request: &AthenaRequest) -> Result<ExecutionModule> {
        let _request_prog = canonicalize_request(request);
        let _plan_prog = plan_from_request(&_request_prog);
        self.lower_module(session, request)
    }

    /// 分阶段编译：具名 Request → Plan →（fused）module → Semantic / CFG SSA。
    pub fn compile_staged(&self, session: &mut Session, request: &AthenaRequest) -> Result<StagedCompile> {
        let request_prog = canonicalize_request(request);
        let plan_prog = plan_from_request(&request_prog);
        let module = self.lower_module(session, request)?;
        let semantic = materialize_semantic(&module);
        let cfg_ssa = materialize_cfg_ssa(&module);
        let observation = CompileObservation::from_programs(
            request_prog.clone(),
            plan_prog.clone(),
            semantic.clone(),
            cfg_ssa.clone(),
        );
        verify_observation(&observation, &module)?;
        Ok(StagedCompile {
            request: request_prog,
            plan: plan_prog,
            semantic,
            cfg_ssa,
            module,
        })
    }

    /// 编译并产出 Living `04` 四阶段可观测 dump（Request / Plan / Semantic / CFG SSA）。
    pub fn compile_observed(
        &self,
        session: &mut Session,
        request: &AthenaRequest,
    ) -> Result<(ExecutionModule, CompileObservation)> {
        let staged = self.compile_staged(session, request)?;
        let observation = CompileObservation::from_programs(
            staged.request,
            staged.plan,
            staged.semantic,
            staged.cfg_ssa,
        );
        Ok((staged.module, observation))
    }

    fn lower_module(&self, session: &mut Session, request: &AthenaRequest) -> Result<ExecutionModule> {
        let mut builder = ModuleBuilder::default();
        let entry = builder.block_id();
        let mut blocks = Vec::new();
        let value = self.lower_request(session, &mut builder, &mut blocks, entry, request)?;
        // 当 lowering 只产生单块返回时，确保入口块存在并返回。
        if blocks.iter().all(|b| b.id != entry) {
            blocks.insert(
                0,
                BasicBlock { id: entry, parameters: Vec::new(), operations: Vec::new(), terminator: Terminator::return_value(value) },
            );
        }
        builder.finish(blocks, entry)
    }

    fn lower_request(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        request: &AthenaRequest,
    ) -> Result<SsaValueId> {
        match request {
            AthenaRequest::Term(term) => self.lower_term(session, builder, blocks, block_id, *term),
            AthenaRequest::Control(plan) => self.lower_control(session, builder, blocks, block_id, plan),
            AthenaRequest::Command(command) => self.lower_command(session, builder, blocks, block_id, command),
            AthenaRequest::Goal(_) => self.lower_goal_provider(builder, blocks, block_id),
        }
    }

    /// Lowering 项请求。控制 / 绑定形式仅经 [`AthenaRequest::Control`]
    /// / [`AthenaRequest::Command`] — 绝不用 Extension 表层名。
    pub(crate) fn lower_term(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        term: TermId,
    ) -> Result<SsaValueId> {
        if matches!(session.arena.get(term), Some(TermNode::Application { head: ApplicationHead::Semantic(SemanticOperator::Hold), .. })) {
            return self.lower_held_term(session, builder, blocks, block_id, term);
        }
        self.lower_term_into_block(session, builder, blocks, block_id, term)
    }

    fn lower_held_term(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        term: TermId,
    ) -> Result<SsaValueId> {
        let root = builder.push_term_root_id(&session.arena, term)?;
        let ssa = builder.ssa();
        blocks.push(BasicBlock {
            id: block_id,
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(ssa),
                result_type: ExecutionValueType::Term,
                kind: OperationKind::LoadTerm { root },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::return_value(ssa),
        });
        Ok(ssa)
    }
}

impl ExecutionCompiler {
    /// 领域目标 lowering 为显式 `CallProvider` + `PublishResult` 边。
    ///
    /// `DomainRequest` 载荷由运行时 `execute_ir_request` 提供
    /// （不存入 module），因此各后端共享同一 IR 形态。
    fn lower_goal_provider(&self, builder: &mut ModuleBuilder, blocks: &mut Vec<BasicBlock>, block_id: BlockId) -> Result<SsaValueId> {
        use athena_types::ExtensionOperatorId;

        let call = builder.push_provider_call(ProviderCallDescriptor::new(ProviderCallId(0), ExtensionOperatorId(0), ExecutionValueType::Unit));
        let effect_call_in = builder.push_effect(EffectKind::CallProvider, None);
        let effect_call_out = builder.push_effect(EffectKind::CallProvider, Some(effect_call_in));
        let payload = builder.ssa();
        let effect_pub_in = builder.push_effect(EffectKind::PublishResult, Some(effect_call_out));
        let effect_pub_out = builder.push_effect(EffectKind::PublishResult, Some(effect_pub_in));
        let published = builder.ssa();
        blocks.push(BasicBlock {
            id: block_id,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    result: Some(payload),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::CallProvider { call, args: Vec::new() },
                    effect_in: Some(effect_call_in),
                    effect_out: Some(effect_call_out),
                },
                Operation {
                    result: Some(published),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::PublishResult { source: payload },
                    effect_in: Some(effect_pub_in),
                    effect_out: Some(effect_pub_out),
                },
            ],
            terminator: Terminator::return_value(published),
        });
        Ok(published)
    }

    fn lower_command(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        command: &SessionCommand,
    ) -> Result<SsaValueId> {
        match command {
            SessionCommand::Define { symbol, value, kind: _, evaluation } => {
                let residual = !matches!(evaluation, BindingEvaluationPolicy::EvaluateBeforeStore);
                if residual {
                    return self.lower_define_capture(session, builder, blocks, block_id, *symbol, *value, *evaluation);
                }
                match session.arena.get(*value) {
                    Some(TermNode::Atom(_)) => self.lower_define_capture(
                        session,
                        builder,
                        blocks,
                        block_id,
                        *symbol,
                        *value,
                        BindingEvaluationPolicy::EvaluateBeforeStore,
                    ),
                    Some(_) => self.lower_define_evaluated(session, builder, blocks, block_id, *symbol, *value),
                    None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                        .detail("component", "ExecutionCompiler")
                        .detail("reason", "missing_term")),
                }
            }
            SessionCommand::RegisterRuleDispatch { table, rule } => {
                let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
                let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
                let unit = builder.ssa();
                blocks.push(BasicBlock {
                    id: block_id,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        result: Some(unit),
                        result_type: ExecutionValueType::Unit,
                        kind: OperationKind::RegisterCompiledRule { table: *table, rule: *rule },
                        effect_in: Some(effect_in),
                        effect_out: Some(effect_out),
                    }],
                    terminator: Terminator::return_value(unit),
                });
                Ok(unit)
            }
            SessionCommand::ClearDefinition { symbol } => {
                let key = builder.ssa();
                let key_constant = builder.push_constant(ConstantValue::symbol(*symbol));
                let unit_const = builder.push_constant(ConstantValue::Unit);
                let unit_val = builder.ssa();
                let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
                let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
                let result = builder.ssa();
                blocks.push(BasicBlock {
                    id: block_id,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            result: Some(key),
                            result_type: ExecutionValueType::Symbol,
                            kind: OperationKind::Constant { constant: key_constant },
                            effect_in: None,
                            effect_out: None,
                        },
                        Operation {
                            result: Some(unit_val),
                            result_type: ExecutionValueType::Unit,
                            kind: OperationKind::Constant { constant: unit_const },
                            effect_in: None,
                            effect_out: None,
                        },
                        Operation {
                            result: Some(result),
                            result_type: ExecutionValueType::Unit,
                            // Unit 右部表示清除绑定（不是把 Unit 存为 Own）。
                            kind: OperationKind::WriteBinding {
                                key,
                                value: unit_val,
                                kind: BindingKind::Session,
                                evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
                            },
                            effect_in: Some(effect_in),
                            effect_out: Some(effect_out),
                        },
                    ],
                    terminator: Terminator::return_value(result),
                });
                Ok(result)
            }
        }
    }
}

impl ExecutionCompiler {
    fn lower_term_into_block(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        term: TermId,
    ) -> Result<SsaValueId> {
        let mut operations = Vec::new();
        let value = self.lower_pure_expr(session, builder, &mut operations, term)?;
        blocks.push(BasicBlock { id: block_id, parameters: Vec::new(), operations, terminator: Terminator::return_value(value) });
        Ok(value)
    }

    /// 将纯原子 / Boolean 语义应用 lowering 为 SSA 操作（无 Session 副作用）。
    fn lower_pure_expr(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        operations: &mut Vec<Operation>,
        term: TermId,
    ) -> Result<SsaValueId> {
        match session.arena.get(term) {
            Some(TermNode::Atom(Atom::Boolean(value))) => {
                let ssa = builder.ssa();
                let constant = builder.push_constant(ConstantValue::boolean(*value));
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            Some(TermNode::Atom(Atom::Symbol(symbol))) => {
                // 用户符号仅作绑定键 — 绝不当显示名常量。
                let key = builder.ssa();
                let key_constant = builder.push_constant(ConstantValue::symbol(*symbol));
                let effect_in = builder.push_effect(EffectKind::ReadBinding, None);
                let effect_out = builder.push_effect(EffectKind::ReadBinding, Some(effect_in));
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(key),
                    result_type: ExecutionValueType::Symbol,
                    kind: OperationKind::Constant { constant: key_constant },
                    effect_in: None,
                    effect_out: None,
                });
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::ReadBinding { key },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                });
                Ok(ssa)
            }
            Some(TermNode::Atom(_)) => {
                let root = builder.push_term_root_id(&session.arena, term)?;
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::LoadTerm { root },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            Some(TermNode::Application { head, arguments }) => {
                let head = *head;
                let arguments = arguments.clone();
                match head {
                    ApplicationHead::Semantic(op) => {
                        let compare_args = if matches!(
                            op,
                            SemanticOperator::Less | SemanticOperator::Greater | SemanticOperator::LessEqual | SemanticOperator::GreaterEqual
                        ) {
                            flatten_compare_chain_args(session, op, term)
                        }
                        else {
                            None
                        };
                        let arg_terms = compare_args.unwrap_or(arguments);
                        let result_type = match op {
                            SemanticOperator::Not
                            | SemanticOperator::And
                            | SemanticOperator::Or
                            | SemanticOperator::TrueQ
                            | SemanticOperator::Identical
                            | SemanticOperator::Equal
                            | SemanticOperator::Unequal
                            | SemanticOperator::Less
                            | SemanticOperator::Greater
                            | SemanticOperator::LessEqual
                            | SemanticOperator::GreaterEqual => ExecutionValueType::Boolean,
                            _ => ExecutionValueType::Term,
                        };
                        // `Hold` / `Function` 不得求值其实参。
                        let hold_all = matches!(op, SemanticOperator::Hold | SemanticOperator::Function);
                        let hold_first = op == SemanticOperator::Product
                            || (op == SemanticOperator::Sum && arg_terms.len() == 2)
                            || matches!(op, SemanticOperator::Apply | SemanticOperator::Map);
                        let hold_second = matches!(op, SemanticOperator::CollectMatches | SemanticOperator::Matches) && arg_terms.len() >= 2;
                        let mut args = Vec::with_capacity(arg_terms.len());
                        for (index, arg) in arg_terms.into_iter().enumerate() {
                            if hold_all || (hold_first && index == 0) || (hold_second && index == 1) {
                                let root = builder.push_term_root_id(&session.arena, arg)?;
                                let ssa = builder.ssa();
                                operations.push(Operation {
                                    result: Some(ssa),
                                    result_type: ExecutionValueType::Term,
                                    kind: OperationKind::LoadTerm { root },
                                    effect_in: None,
                                    effect_out: None,
                                });
                                args.push(ssa);
                            }
                            else {
                                args.push(self.lower_pure_expr(session, builder, operations, arg)?);
                            }
                        }
                        let ssa = builder.ssa();
                        operations.push(Operation {
                            result: Some(ssa),
                            result_type,
                            kind: OperationKind::ApplySemanticOperator { operator: op, args },
                            effect_in: None,
                            effect_out: None,
                        });
                        Ok(ssa)
                    }
                    ApplicationHead::Extension(ext) => {
                        let mut args = Vec::with_capacity(arguments.len());
                        for arg in arguments {
                            args.push(self.lower_pure_expr(session, builder, operations, arg)?);
                        }
                        let ssa = builder.ssa();
                        operations.push(Operation {
                            result: Some(ssa),
                            result_type: ExecutionValueType::Term,
                            kind: OperationKind::ApplyExtensionOperator { operator: ext, args },
                            effect_in: None,
                            effect_out: None,
                        });
                        Ok(ssa)
                    }
                }
            }
            Some(TermNode::Collection { kind: coll_kind, elements: items }) => {
                let coll_kind = *coll_kind;
                let items = items.clone();
                let mut elements = Vec::with_capacity(items.len());
                for item in items {
                    elements.push(self.lower_pure_expr(session, builder, operations, item)?);
                }
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::ConstructCollection { kind: coll_kind, elements },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            None => {
                Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("component", "ExecutionCompiler").detail("reason", "missing_term"))
            }
        }
    }

    fn require_atom(&self, session: &mut Session, term: TermId) -> Result<()> {
        match session.arena.get(term) {
            Some(TermNode::Atom(_)) => Ok(()),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "compound_term_not_lowered")),
            None => {
                Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("component", "ExecutionCompiler").detail("reason", "missing_term"))
            }
        }
    }

    fn require_boolean_atom(&self, session: &mut Session, term: TermId) -> Result<bool> {
        match session.arena.get(term) {
            Some(TermNode::Atom(Atom::Boolean(value))) => Ok(*value),
            // 精确 `0`/`1` 真值。其他数字失败，以便
            // `Branch` 回退到运行时谓词 lowering。
            Some(TermNode::Atom(Atom::Number(n))) => {
                if n.is_zero() {
                    Ok(false)
                }
                else if *n == athena_numeric::Number::small_int(1) {
                    Ok(true)
                }
                else {
                    Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "branch_condition_not_boolean_atom"))
                }
            }
            Some(TermNode::Atom(Atom::Null)) => Ok(false),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "branch_condition_not_boolean_atom")),
            None => {
                Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("component", "ExecutionCompiler").detail("reason", "missing_term"))
            }
        }
    }
}
