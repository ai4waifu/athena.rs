//! Define / binding lowering for [`super::ExecutionCompiler`].

use athena_ir::{Atom, TermNode};
use athena_types::{BindingEvaluationPolicy, BindingKind, Diagnostic, DiagnosticCode, Result, TermId};

use super::{ExecutionCompiler, ModuleBuilder};
use crate::{
    api::request::{AthenaRequest, ControlPlan, SessionCommand},
    execution::ir::{
        BasicBlock, BlockEdge, BlockId, ConstantValue, EffectKind, ExecutionValueType, Operation, OperationKind, SsaValueId, Terminator,
    },
    runtime::session::Session,
};

impl ExecutionCompiler {
    /// Capture pattern lhs + residual rhs then `RegisterRuleDispatch`.
    pub(crate) fn lower_register_rule(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        symbol: athena_types::SymbolId,
        pattern: TermId,
        value: TermId,
    ) -> Result<SsaValueId> {
        let _ = session;
        let key = builder.ssa();
        let key_constant = builder.push_constant(ConstantValue::symbol(symbol));
        let pattern_root = builder.push_term_root(pattern);
        let pattern_ssa = builder.ssa();
        let value_root = builder.push_term_root(value);
        let value_ssa = builder.ssa();
        let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
        let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
        let unit = builder.ssa();
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
                    result: Some(pattern_ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::LoadTerm { root: pattern_root },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(value_ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::LoadTerm { root: value_root },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(unit),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::RegisterRuleDispatch { head: key, pattern: pattern_ssa, replacement: value_ssa },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                },
            ],
            terminator: Terminator::return_value(unit),
        });
        Ok(unit)
    }

    /// Capture rhs as `LoadTerm` then `WriteBinding` (atoms / Deferred compounds).
    pub(crate) fn lower_define_capture(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        symbol: athena_types::SymbolId,
        value: TermId,
        evaluation: BindingEvaluationPolicy,
    ) -> Result<SsaValueId> {
        let _ = session;
        let key = builder.ssa();
        let key_constant = builder.push_constant(ConstantValue::symbol(symbol));
        let root = builder.push_term_root(value);
        let rhs = builder.ssa();
        let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
        let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
        let unit = builder.ssa();
        let residual = !matches!(evaluation, BindingEvaluationPolicy::EvaluateBeforeStore);
        let returned = if residual { unit } else { rhs };
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
                    result: Some(rhs),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::LoadTerm { root },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(unit),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::WriteBinding {
                        key,
                        value: rhs,
                        kind: BindingKind::Session,
                        evaluation,
                    },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                },
            ],
            terminator: Terminator::return_value(returned),
        });
        Ok(returned)
    }

    /// Immediate `Define` with compound rhs: evaluate then bind the result.
    pub(crate) fn lower_define_evaluated(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        symbol: athena_types::SymbolId,
        rhs: TermId,
    ) -> Result<SsaValueId> {
        let eval_block = builder.block_id();
        let bind_block = builder.block_id();
        let entry_cond = builder.ssa();
        let entry_true = builder.push_constant(ConstantValue::boolean(true));

        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(entry_cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: entry_true },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::Branch {
                condition: entry_cond,
                then_edge: BlockEdge::jump(eval_block),
                else_edge: BlockEdge::jump(eval_block),
            },
        });

        let rhs_value = self.lower_request(session, builder, blocks, eval_block, &AthenaRequest::Term(rhs))?;
        self.rewrite_returns_to_join(builder, blocks, bind_block, rhs_value)?;

        let value_param = builder.ssa();
        let key = builder.ssa();
        let key_constant = builder.push_constant(ConstantValue::symbol(symbol));
        let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
        let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
        let unit = builder.ssa();
        blocks.push(BasicBlock {
            id: bind_block,
            parameters: vec![crate::execution::ir::BlockParameter { value: value_param, ty: ExecutionValueType::Term }],
            operations: vec![
                Operation {
                    result: Some(key),
                    result_type: ExecutionValueType::Symbol,
                    kind: OperationKind::Constant { constant: key_constant },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(unit),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::WriteBinding {
                        key,
                        value: value_param,
                        kind: BindingKind::Session,
                        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
                    },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                },
            ],
            terminator: Terminator::return_value(value_param),
        });
        Ok(value_param)
    }

    /// Term `LocalScope` / `LexicalScope` / `DynamicScope`: locals list + body inside `EnterScope`.
    pub(crate) fn lower_term_scope(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        head: &str,
        locals: TermId,
        body: TermId,
    ) -> Result<SsaValueId> {
        let items = match session.arena.get(locals) {
            Some(TermNode::Collection { elements: items, .. }) => items.clone(),
            _ => {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionCompiler")
                    .detail("status", "scope_locals_not_list"));
            }
        };
        let allow_bare = head == "LexicalScope";
        let mut steps = Vec::with_capacity(items.len().saturating_add(1));
        for item in items {
            if let Some((symbol, rhs)) = self.match_define_term(session, item) {
                steps.push(AthenaRequest::Command(SessionCommand::Define {
                    symbol,
                    value: rhs,
                    kind: BindingKind::Session,
                    evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
                }));
                continue;
            }
            if allow_bare {
                if let Some(TermNode::Atom(Atom::Symbol(symbol))) = session.arena.get(item) {
                    let symbol = *symbol;
                    let name = session.arena.symbols().resolve(symbol).unwrap_or("x").to_string();
                    session.module_counter = session.module_counter.saturating_add(1);
                    let uniq = format!("{name}${}", session.module_counter);
                    let uniq_term = session.builder().symbol(&uniq, Default::default());
                    steps.push(AthenaRequest::Command(SessionCommand::Define {
                        symbol,
                        value: uniq_term,
                        kind: BindingKind::Session,
                    evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
                    }));
                    continue;
                }
            }
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "scope_local_not_supported"));
        }
        steps.push(AthenaRequest::Term(body));
        self.lower_scope(session, builder, blocks, block_id, &AthenaRequest::Control(ControlPlan::Sequence { steps }))
    }

    pub(crate) fn match_define_term(&self, session: &Session, term: TermId) -> Option<(athena_types::SymbolId, TermId)> {
        let TermNode::Application { head, arguments } = session.arena.get(term)?
        else {
            return None;
        };
        if arguments.len() != 2 {
            return None;
        }
        let name = match *head {
            athena_ir::ApplicationHead::Extension(id) => session.operators.name(id)?,
            athena_ir::ApplicationHead::Semantic(_) => return None,
        };
        if name != "Define" {
            return None;
        }
        match session.arena.get(arguments[0]) {
            Some(TermNode::Atom(Atom::Symbol(symbol))) => Some((*symbol, arguments[1])),
            _ => None,
        }
    }

    pub(crate) fn lower_term_cond(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        arguments: &[TermId],
    ) -> Result<SsaValueId> {
        if arguments.len() % 2 != 0 {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "cond_arity_not_pairs"));
        }
        let mut arms: Vec<(TermId, Box<AthenaRequest>)> = Vec::with_capacity(arguments.len() / 2);
        for pair in arguments.chunks_exact(2) {
            arms.push((pair[0], Box::new(AthenaRequest::Term(pair[1]))));
        }
        self.lower_cond(session, builder, blocks, block_id, &arms, None)
    }
}
