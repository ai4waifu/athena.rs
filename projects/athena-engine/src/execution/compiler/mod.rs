//! `ExecutionCompiler` — `AthenaRequest` + Session snapshot → [`ExecutionModule`].
//!
//! Bootstrap lowering: atom terms, typed Boolean constants, `ControlPlan::Branch` /
//! `Sequence`, and effectful `SessionCommand::Define` via `WriteBinding`.
//! No bridge to a deleted stack interpreter.

use athena_ir::{Atom, TermNode};
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

use crate::{
    api::request::{AthenaRequest, ControlPlan, DefinitionEvaluationTiming, SessionCommand},
    execution::ir::{
        BasicBlock, BlockEdge, BlockId, CapturedRoot, CapturedRootId, ConstantId, ConstantValue, EffectEdge, EffectKind, EffectToken,
        ExecutionModule, ExecutionValueType, ModuleFingerprint, Operation, OperationKind, ProviderCallDescriptor, ProviderCallId, Region,
        RegionId, SsaValueId, Terminator, verify_module,
    },
    runtime::session::Session,
};

/// Compiles one request into a verified [`ExecutionModule`].
#[derive(Debug, Default)]
pub struct ExecutionCompiler {}


mod builder;
mod helpers;

use builder::ModuleBuilder;
use helpers::{collect_compare_chain_args, expand_span_range, flatten_compare_chain_args};

impl ExecutionCompiler {
    /// Create a compiler instance.
    pub fn new() -> Self {
        Self {}
    }

    /// Lower a request against a Session snapshot into `ExecutionIR`.
    pub fn compile(&self, session: &mut Session, request: &AthenaRequest) -> Result<ExecutionModule> {
        let mut builder = ModuleBuilder::default();
        let entry = builder.block_id();
        let mut blocks = Vec::new();
        let value = self.lower_request(session, &mut builder, &mut blocks, entry, request)?;
        // Ensure entry block exists and returns when lowering produced a single block return.
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

    /// Lower a term request: `If` / `Sequence` / `Hold` get special forms, others fill one block.
    fn lower_term(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        term: TermId,
    ) -> Result<SsaValueId> {
        let app = match session.arena.get(term) {
            Some(TermNode::Application { head, arguments }) => Some((session.operators.name(*head).map(str::to_owned), arguments.clone())),
            _ => None,
        };
        if let Some((name, arguments)) = app {
            let name = name.as_deref();
            if name == Some("If") || name == Some("Branch") {
                return match arguments.as_slice() {
                    [condition, then_branch] => {
                        let then_req = AthenaRequest::Term(*then_branch);
                        self.lower_branch(session, builder, blocks, block_id, *condition, &then_req, None)
                    }
                    [condition, then_branch, else_branch] => {
                        let then_req = AthenaRequest::Term(*then_branch);
                        let else_req = AthenaRequest::Term(*else_branch);
                        self.lower_branch(session, builder, blocks, block_id, *condition, &then_req, Some(&else_req))
                    }
                    _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "if_arity_not_supported")),
                };
            }
            if name == Some("Define") {
                return match arguments.as_slice() {
                    [lhs, rhs] => {
                        let symbol = match session.arena.get(*lhs) {
                            Some(TermNode::Atom(Atom::Symbol(symbol))) => Some(*symbol),
                            _ => None,
                        };
                        match symbol {
                            Some(symbol) => self.lower_command(
                                session,
                                builder,
                                blocks,
                                block_id,
                                &SessionCommand::Define { symbol, value: *rhs, timing: DefinitionEvaluationTiming::Immediate },
                            ),
                            None => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                                .detail("component", "ExecutionCompiler")
                                .detail("status", "define_lhs_not_symbol")),
                        }
                    }
                    _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "define_arity_not_supported")),
                };
            }
            if name == Some("DefineDeferred") {
                return match arguments.as_slice() {
                    [lhs, rhs] => match session.arena.get(*lhs) {
                        Some(TermNode::Atom(Atom::Symbol(symbol))) => self.lower_command(
                            session,
                            builder,
                            blocks,
                            block_id,
                            &SessionCommand::Define { symbol: *symbol, value: *rhs, timing: DefinitionEvaluationTiming::Deferred },
                        ),
                        Some(TermNode::Application { head: op, .. }) => {
                            let head_name = session.operators.name(*op).unwrap_or("").to_string();
                            if head_name.is_empty() || head_name == "Application" {
                                Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                                    .detail("component", "ExecutionCompiler")
                                    .detail("status", "define_deferred_lhs_not_supported"))
                            }
                            else {
                                let symbol = session.arena.symbols_mut().intern(&head_name);
                                self.lower_define_down_value(session, builder, blocks, block_id, symbol, *lhs, *rhs)
                            }
                        }
                        _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                            .detail("component", "ExecutionCompiler")
                            .detail("status", "define_deferred_lhs_not_supported")),
                    },
                    _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "define_deferred_arity_not_supported")),
                };
            }
            if name == Some("LocalScope") || name == Some("LexicalScope") || name == Some("DynamicScope") {
                return match arguments.as_slice() {
                    [locals, body] => self.lower_term_scope(session, builder, blocks, block_id, name.unwrap_or("LocalScope"), *locals, *body),
                    _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "scope_arity_not_supported")),
                };
            }
            if name == Some("Recover") {
                return match arguments.as_slice() {
                    [body, handler] => {
                        self.lower_recover(session, builder, blocks, block_id, &AthenaRequest::Term(*body), &AthenaRequest::Term(*handler))
                    }
                    _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "recover_arity_not_supported")),
                };
            }
            if name == Some("error") || name == Some("Error") {
                return self.lower_error_reject(builder, blocks, block_id);
            }
            if name == Some("Cond") {
                return self.lower_term_cond(session, builder, blocks, block_id, &arguments);
            }
            if name == Some("CountedLoop") {
                return match arguments.as_slice() {
                    [variable, iterator, body] => {
                        self.lower_counted_loop(session, builder, blocks, block_id, *variable, *iterator, &AthenaRequest::Term(*body))
                    }
                    _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "counted_loop_arity_not_supported")),
                };
            }
            if name == Some("LoopWhile") {
                return match arguments.as_slice() {
                    [condition, body] => self.lower_loop_while(session, builder, blocks, block_id, *condition, &AthenaRequest::Term(*body)),
                    _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "loop_while_arity_not_supported")),
                };
            }
            if name == Some("Sequence") {
                let steps: Vec<AthenaRequest> = arguments.iter().copied().map(AthenaRequest::Term).collect();
                return self.lower_sequence(session, builder, blocks, block_id, &steps);
            }
            if name == Some("Hold") {
                // Capture the whole held term without evaluating arguments.
                return self.lower_held_term(builder, blocks, block_id, term);
            }
        }
        self.lower_term_into_block(session, builder, blocks, block_id, term)
    }

    fn lower_held_term(
        &self,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        term: TermId,
    ) -> Result<SsaValueId> {
        let root = builder.push_term_root(term);
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

    /// Lower `error`/`Error` as a hard `Reject` so `Recover` can catch it.
    fn lower_error_reject(&self, builder: &mut ModuleBuilder, blocks: &mut Vec<BasicBlock>, block_id: BlockId) -> Result<SsaValueId> {
        let placeholder = builder.ssa();
        let constant = builder.push_constant(ConstantValue::Unit);
        blocks.push(BasicBlock {
            id: block_id,
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(placeholder),
                result_type: ExecutionValueType::Unit,
                kind: OperationKind::Constant { constant },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::Reject { exit: None },
        });
        Ok(placeholder)
    }

    /// Capture pattern lhs + deferred rhs then `WriteDownValue`.
    fn lower_define_down_value(
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
                    kind: OperationKind::WriteDownValue { key, pattern: pattern_ssa, value: value_ssa },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                },
            ],
            terminator: Terminator::return_value(unit),
        });
        Ok(unit)
    }

    /// Capture rhs as `LoadTerm` then `WriteBinding` (atoms / Deferred compounds).
    fn lower_define_capture(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        symbol: athena_types::SymbolId,
        value: TermId,
        delayed: bool,
    ) -> Result<SsaValueId> {
        let _ = session;
        let key = builder.ssa();
        let key_constant = builder.push_constant(ConstantValue::symbol(symbol));
        let root = builder.push_term_root(value);
        let rhs = builder.ssa();
        let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
        let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
        let unit = builder.ssa();
        // Immediate returns rhs; Deferred returns Null.
        let returned = if delayed { unit } else { rhs };
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
                    kind: OperationKind::WriteBinding { key, value: rhs, delayed },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                },
            ],
            terminator: Terminator::return_value(returned),
        });
        Ok(returned)
    }

    /// Immediate `Define` with compound rhs: evaluate then bind the result.
    fn lower_define_evaluated(
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
                    kind: OperationKind::WriteBinding { key, value: value_param, delayed: false },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                },
            ],
            terminator: Terminator::return_value(value_param),
        });
        Ok(value_param)
    }

    /// Term `LocalScope` / `LexicalScope` / `DynamicScope`: locals list + body inside `EnterScope`.
    fn lower_term_scope(
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
            Some(TermNode::List(items)) => items.clone(),
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
                    timing: DefinitionEvaluationTiming::Immediate,
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
                        timing: DefinitionEvaluationTiming::Immediate,
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

    fn match_define_term(&self, session: &Session, term: TermId) -> Option<(athena_types::SymbolId, TermId)> {
        let TermNode::Application { head, arguments } = session.arena.get(term)?
        else {
            return None;
        };
        if arguments.len() != 2 {
            return None;
        }
        let name = session.operators.name(*head)?;
        if name != "Define" {
            return None;
        }
        match session.arena.get(arguments[0]) {
            Some(TermNode::Atom(Atom::Symbol(symbol))) => Some((*symbol, arguments[1])),
            _ => None,
        }
    }

    fn lower_term_cond(
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

    /// Domain goals lower to an explicit `CallProvider` + `PublishResult` edge.
    ///
    /// The `DomainRequest` payload is supplied at runtime by `execute_ir_request`
    /// (not stored in the module), so backends share the same IR shape.
    fn lower_goal_provider(&self, builder: &mut ModuleBuilder, blocks: &mut Vec<BasicBlock>, block_id: BlockId) -> Result<SsaValueId> {
        use athena_types::OperatorId;

        let call = builder.push_provider_call(ProviderCallDescriptor::new(ProviderCallId(0), OperatorId(0), ExecutionValueType::Unit));
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
            SessionCommand::Define { symbol, value, timing } => {
                let delayed = matches!(timing, DefinitionEvaluationTiming::Deferred);
                if delayed {
                    return self.lower_define_capture(session, builder, blocks, block_id, *symbol, *value, true);
                }
                // Immediate: atoms bind directly; compounds evaluate then bind (VM Set parity).
                match session.arena.get(*value) {
                    Some(TermNode::Atom(_)) => self.lower_define_capture(session, builder, blocks, block_id, *symbol, *value, false),
                    Some(_) => self.lower_define_evaluated(session, builder, blocks, block_id, *symbol, *value),
                    None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                        .detail("component", "ExecutionCompiler")
                        .detail("reason", "missing_term")),
                }
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
                            // Unit rhs means clear binding (not store Unit as Own).
                            kind: OperationKind::WriteBinding { key, value: unit_val, delayed: false },
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

    fn lower_control(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        plan: &ControlPlan,
    ) -> Result<SsaValueId> {
        match plan {
            ControlPlan::Sequence { steps } => self.lower_sequence(session, builder, blocks, block_id, steps),
            ControlPlan::Branch { condition, then_branch, else_branch } => {
                self.lower_branch(session, builder, blocks, block_id, *condition, then_branch, else_branch.as_deref())
            }
            ControlPlan::LocalScope { body } | ControlPlan::LexicalScope { body } | ControlPlan::DynamicScope { body } => {
                self.lower_scope(session, builder, blocks, block_id, body)
            }
            ControlPlan::Cond { arms, otherwise } => self.lower_cond(session, builder, blocks, block_id, arms, otherwise.as_deref()),
            ControlPlan::Recover { body, handler } => self.lower_recover(session, builder, blocks, block_id, body, handler),
            ControlPlan::LoopWhile { condition, body } => self.lower_loop_while(session, builder, blocks, block_id, *condition, body),
            ControlPlan::CountedLoop { variable, iterator, body } => {
                self.lower_counted_loop(session, builder, blocks, block_id, *variable, *iterator, body)
            }
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "control_plan_not_lowered")),
        }
    }

    fn lower_counted_loop(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        variable: TermId,
        iterator: TermId,
        body: &AthenaRequest,
    ) -> Result<SsaValueId> {
        let symbol = self.require_symbol_atom(session, variable)?;
        let items = self.require_atom_list(session, iterator)?;
        let AthenaRequest::Term(body_term) = body
        else {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "counted_loop_body_must_be_term"));
        };

        if items.is_empty() {
            let value = builder.ssa();
            let constant = builder.push_constant(ConstantValue::Unit);
            blocks.push(BasicBlock {
                id: entry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    result: Some(value),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::Constant { constant },
                    effect_in: None,
                    effect_out: None,
                }],
                terminator: Terminator::return_value(value),
            });
            return Ok(value);
        }

        // Bootstrap: unroll constant atom lists into Define + body Term steps.
        let mut steps = Vec::with_capacity(items.len().saturating_mul(2));
        for item in items {
            steps.push(AthenaRequest::Command(SessionCommand::Define { symbol, value: item, timing: DefinitionEvaluationTiming::Immediate }));
            steps.push(AthenaRequest::Term(*body_term));
        }
        let budget_in = builder.push_effect(EffectKind::BudgetCheck, None);
        let _budget_out = builder.push_effect(EffectKind::BudgetCheck, Some(budget_in));
        self.lower_sequence(session, builder, blocks, entry, &steps)
    }

    fn require_symbol_atom(&self, session: &mut Session, term: TermId) -> Result<athena_types::SymbolId> {
        match session.arena.get(term) {
            Some(TermNode::Atom(Atom::Symbol(symbol))) => Ok(*symbol),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "counted_loop_variable_not_symbol")),
            None => {
                Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("component", "ExecutionCompiler").detail("reason", "missing_term"))
            }
        }
    }

    fn require_atom_list(&self, session: &mut Session, term: TermId) -> Result<Vec<TermId>> {
        let list_items = match session.arena.get(term) {
            Some(TermNode::List(items)) => Some(items.clone()),
            _ => None,
        };
        if let Some(items) = list_items {
            for item in &items {
                self.require_atom(session, *item)?;
            }
            return Ok(items);
        }
        let span_args = match session.arena.get(term) {
            Some(TermNode::Application { head, arguments }) if session.operators.name(*head) == Some("Span") => Some(arguments.clone()),
            _ => None,
        };
        if let Some(arguments) = span_args {
            return self.expand_span_iterator(session, &arguments);
        }
        match session.arena.get(term) {
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "counted_loop_iterator_not_atom_list")),
            None => {
                Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("component", "ExecutionCompiler").detail("reason", "missing_term"))
            }
        }
    }

    fn expand_span_iterator(&self, session: &mut Session, arguments: &[TermId]) -> Result<Vec<TermId>> {
        let ints: Option<Vec<i64>> = arguments
            .iter()
            .map(|t| match session.arena.get(*t) {
                Some(TermNode::Atom(Atom::Number(n))) => n.as_exact_integer(),
                _ => None,
            })
            .collect();
        let Some(ints) = ints
        else {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "span_bounds_not_integer"));
        };
        let values = match ints.as_slice() {
            [a, b] => expand_span_range(*a, 1, *b),
            [a, step, b] => expand_span_range(*a, *step, *b),
            _ => {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionCompiler")
                    .detail("status", "span_arity_not_supported"));
            }
        };
        let Some(values) = values
        else {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "span_range_invalid"));
        };
        Ok(values.into_iter().map(|v| session.builder().int(v, Default::default())).collect())
    }

    fn lower_loop_while(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        condition: TermId,
        body: &AthenaRequest,
    ) -> Result<SsaValueId> {
        let header = builder.block_id();
        let body_block = builder.block_id();
        let exit = builder.block_id();
        let acc_param = builder.ssa();
        let exit_param = builder.ssa();
        let init = builder.ssa();
        let init_const = builder.push_constant(ConstantValue::Unit);
        let entry_cond = builder.ssa();
        let entry_true = builder.push_constant(ConstantValue::boolean(true));
        let budget_in = builder.push_effect(EffectKind::BudgetCheck, None);
        let budget_out = builder.push_effect(EffectKind::BudgetCheck, Some(budget_in));

        // entry → header(Unit)
        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    result: Some(init),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::Constant { constant: init_const },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(entry_cond),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: entry_true },
                    effect_in: Some(budget_in),
                    effect_out: Some(budget_out),
                },
            ],
            terminator: Terminator::Branch {
                condition: entry_cond,
                then_edge: BlockEdge { target: header, arguments: vec![init] },
                else_edge: BlockEdge { target: header, arguments: vec![init] },
            },
        });

        // Header re-evaluates the predicate each iteration (ReadBinding / compares).
        let mut header_ops = Vec::new();
        let loop_cond = match self.require_boolean_atom(session, condition) {
            Ok(cond_bool) => {
                let loop_cond = builder.ssa();
                let loop_const = builder.push_constant(ConstantValue::boolean(cond_bool));
                header_ops.push(Operation {
                    result: Some(loop_cond),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: loop_const },
                    effect_in: None,
                    effect_out: None,
                });
                loop_cond
            }
            Err(_) => self.lower_pure_expr(session, builder, &mut header_ops, condition)?,
        };
        blocks.push(BasicBlock {
            id: header,
            parameters: vec![crate::execution::ir::BlockParameter { value: acc_param, ty: ExecutionValueType::Term }],
            operations: header_ops,
            terminator: Terminator::Branch {
                condition: loop_cond,
                then_edge: BlockEdge::jump(body_block),
                else_edge: BlockEdge { target: exit, arguments: vec![acc_param] },
            },
        });

        let body_value = self.lower_request(session, builder, blocks, body_block, body)?;
        // Body returns continue at header with the new accumulator.
        self.rewrite_returns_to_join(builder, blocks, header, body_value)?;

        blocks.push(BasicBlock {
            id: exit,
            parameters: vec![crate::execution::ir::BlockParameter { value: exit_param, ty: ExecutionValueType::Term }],
            operations: Vec::new(),
            terminator: Terminator::return_value(exit_param),
        });
        Ok(exit_param)
    }

    fn lower_recover(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        body: &AthenaRequest,
        handler: &AthenaRequest,
    ) -> Result<SsaValueId> {
        let body_block = builder.block_id();
        let handler_block = builder.block_id();
        let join = builder.block_id();
        let result_param = builder.ssa();
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
                then_edge: BlockEdge::jump(body_block),
                else_edge: BlockEdge::jump(body_block),
            },
        });

        let body_value = self.lower_request(session, builder, blocks, body_block, body)?;
        self.rewrite_rejects_to_handler(builder, blocks, handler_block)?;
        self.rewrite_returns_to_join(builder, blocks, join, body_value)?;

        let handler_value = self.lower_request(session, builder, blocks, handler_block, handler)?;
        self.rewrite_returns_to_join(builder, blocks, join, handler_value)?;

        blocks.push(BasicBlock {
            id: join,
            parameters: vec![crate::execution::ir::BlockParameter { value: result_param, ty: ExecutionValueType::Term }],
            operations: Vec::new(),
            terminator: Terminator::return_value(result_param),
        });
        Ok(result_param)
    }

    fn rewrite_rejects_to_handler(&self, builder: &mut ModuleBuilder, blocks: &mut Vec<BasicBlock>, handler: BlockId) -> Result<()> {
        let reject_ids: Vec<BlockId> = blocks.iter().filter(|b| matches!(b.terminator, Terminator::Reject { .. })).map(|b| b.id).collect();
        for block_id in reject_ids {
            let cond = builder.ssa();
            let true_const = builder.push_constant(ConstantValue::boolean(true));
            let block = blocks.iter_mut().find(|b| b.id == block_id).expect("block");
            block.operations.push(Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: true_const },
                effect_in: None,
                effect_out: None,
            });
            block.terminator = Terminator::Branch { condition: cond, then_edge: BlockEdge::jump(handler), else_edge: BlockEdge::jump(handler) };
        }
        Ok(())
    }

    fn lower_cond(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        arms: &[(TermId, Box<AthenaRequest>)],
        otherwise: Option<&AthenaRequest>,
    ) -> Result<SsaValueId> {
        if arms.is_empty() {
            return match otherwise {
                Some(request) => self.lower_request(session, builder, blocks, entry, request),
                None => {
                    let value = builder.ssa();
                    let constant = builder.push_constant(ConstantValue::Unit);
                    blocks.push(BasicBlock {
                        id: entry,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            result: Some(value),
                            result_type: ExecutionValueType::Unit,
                            kind: OperationKind::Constant { constant },
                            effect_in: None,
                            effect_out: None,
                        }],
                        terminator: Terminator::return_value(value),
                    });
                    Ok(value)
                }
            };
        }

        let join = builder.block_id();
        let result_param = builder.ssa();
        let mut test_blocks = Vec::with_capacity(arms.len());
        test_blocks.push(entry);
        for _ in 1..arms.len() {
            test_blocks.push(builder.block_id());
        }
        let otherwise_block = builder.block_id();

        for (index, (condition, arm)) in arms.iter().enumerate() {
            let mut operations = Vec::new();
            let cond_value = match self.require_boolean_atom(session, *condition) {
                Ok(cond_bool) => {
                    let cond_value = builder.ssa();
                    let cond_constant = builder.push_constant(ConstantValue::boolean(cond_bool));
                    operations.push(Operation {
                        result: Some(cond_value),
                        result_type: ExecutionValueType::Boolean,
                        kind: OperationKind::Constant { constant: cond_constant },
                        effect_in: None,
                        effect_out: None,
                    });
                    cond_value
                }
                Err(_) => self.lower_pure_expr(session, builder, &mut operations, *condition)?,
            };
            let arm_block = builder.block_id();
            let else_target = if index + 1 < arms.len() { test_blocks[index + 1] } else { otherwise_block };
            blocks.push(BasicBlock {
                id: test_blocks[index],
                parameters: Vec::new(),
                operations,
                terminator: Terminator::Branch {
                    condition: cond_value,
                    then_edge: BlockEdge::jump(arm_block),
                    else_edge: BlockEdge::jump(else_target),
                },
            });
            let arm_value = self.lower_request(session, builder, blocks, arm_block, arm)?;
            self.rewrite_returns_to_join(builder, blocks, join, arm_value)?;
        }

        match otherwise {
            Some(request) => {
                let other_value = self.lower_request(session, builder, blocks, otherwise_block, request)?;
                self.rewrite_returns_to_join(builder, blocks, join, other_value)?;
            }
            None => {
                let value = builder.ssa();
                let constant = builder.push_constant(ConstantValue::Unit);
                blocks.push(BasicBlock {
                    id: otherwise_block,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        result: Some(value),
                        result_type: ExecutionValueType::Unit,
                        kind: OperationKind::Constant { constant },
                        effect_in: None,
                        effect_out: None,
                    }],
                    terminator: Terminator::Return { values: vec![value] },
                });
                self.rewrite_returns_to_join(builder, blocks, join, value)?;
            }
        }

        blocks.push(BasicBlock {
            id: join,
            parameters: vec![crate::execution::ir::BlockParameter { value: result_param, ty: ExecutionValueType::Term }],
            operations: Vec::new(),
            terminator: Terminator::return_value(result_param),
        });
        Ok(result_param)
    }

    /// Rewrite current `Return` terminators into jumps to `join` with a block argument.
    fn rewrite_returns_to_join(
        &self,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        join: BlockId,
        fallback: SsaValueId,
    ) -> Result<()> {
        let return_block_ids: Vec<BlockId> =
            blocks.iter().filter(|b| matches!(b.terminator, Terminator::Return { .. })).map(|b| b.id).collect();
        for block_id in return_block_ids {
            let forwarded = {
                let block = blocks.iter().find(|b| b.id == block_id).expect("block");
                match &block.terminator {
                    Terminator::Return { values } => values.first().copied().unwrap_or(fallback),
                    _ => fallback,
                }
            };
            let cond = builder.ssa();
            let true_const = builder.push_constant(ConstantValue::boolean(true));
            let block = blocks.iter_mut().find(|b| b.id == block_id).expect("block");
            block.operations.push(Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: true_const },
                effect_in: None,
                effect_out: None,
            });
            block.terminator = Terminator::Branch {
                condition: cond,
                then_edge: BlockEdge { target: join, arguments: vec![forwarded] },
                else_edge: BlockEdge { target: join, arguments: vec![forwarded] },
            };
        }
        Ok(())
    }

    fn lower_scope(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        body: &AthenaRequest,
    ) -> Result<SsaValueId> {
        let body_block = builder.block_id();
        let exit_block = builder.block_id();
        let scope = builder.ssa();
        let enter_in = builder.push_effect(EffectKind::EnterScope, None);
        let enter_out = builder.push_effect(EffectKind::EnterScope, Some(enter_in));
        let entry_cond = builder.ssa();
        let entry_true = builder.push_constant(ConstantValue::boolean(true));

        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    result: Some(scope),
                    result_type: ExecutionValueType::Scope,
                    kind: OperationKind::EnterScope { parent: None },
                    effect_in: Some(enter_in),
                    effect_out: Some(enter_out),
                },
                Operation {
                    result: Some(entry_cond),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: entry_true },
                    effect_in: None,
                    effect_out: None,
                },
            ],
            terminator: Terminator::Branch {
                condition: entry_cond,
                then_edge: BlockEdge::jump(body_block),
                else_edge: BlockEdge::jump(body_block),
            },
        });

        let body_value = self.lower_request(session, builder, blocks, body_block, body)?;
        // Any body Return (including Sequence tails) continues to ExitScope.
        let return_block_ids: Vec<BlockId> =
            blocks.iter().filter(|b| matches!(b.terminator, Terminator::Return { .. })).map(|b| b.id).collect();
        for block_id in return_block_ids {
            let forwarded = {
                let block = blocks.iter().find(|b| b.id == block_id).expect("block");
                match &block.terminator {
                    Terminator::Return { values } => values.first().copied().unwrap_or(body_value),
                    _ => body_value,
                }
            };
            let cond = builder.ssa();
            let true_const = builder.push_constant(ConstantValue::boolean(true));
            let block = blocks.iter_mut().find(|b| b.id == block_id).expect("block");
            block.operations.push(Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: true_const },
                effect_in: None,
                effect_out: None,
            });
            block.terminator = Terminator::Branch {
                condition: cond,
                then_edge: BlockEdge { target: exit_block, arguments: vec![forwarded] },
                else_edge: BlockEdge { target: exit_block, arguments: vec![forwarded] },
            };
        }

        let result_param = builder.ssa();
        let exit_in = builder.push_effect(EffectKind::ExitScope, Some(enter_out));
        let exit_out = builder.push_effect(EffectKind::ExitScope, Some(exit_in));
        blocks.push(BasicBlock {
            id: exit_block,
            parameters: vec![crate::execution::ir::BlockParameter { value: result_param, ty: ExecutionValueType::Term }],
            operations: vec![Operation {
                result: None,
                result_type: ExecutionValueType::Unit,
                kind: OperationKind::ExitScope { scope },
                effect_in: Some(exit_in),
                effect_out: Some(exit_out),
            }],
            terminator: Terminator::return_value(result_param),
        });
        Ok(result_param)
    }

    fn lower_sequence(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        steps: &[AthenaRequest],
    ) -> Result<SsaValueId> {
        if steps.is_empty() {
            let value = builder.ssa();
            let constant = builder.push_constant(ConstantValue::Unit);
            blocks.push(BasicBlock {
                id: entry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    result: Some(value),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::Constant { constant },
                    effect_in: None,
                    effect_out: None,
                }],
                terminator: Terminator::return_value(value),
            });
            return Ok(value);
        }

        let mut step_blocks = Vec::with_capacity(steps.len());
        step_blocks.push(entry);
        for _ in 1..steps.len() {
            step_blocks.push(builder.block_id());
        }

        let mut last_value = SsaValueId(0);
        for (index, step) in steps.iter().enumerate() {
            let block_id = step_blocks[index];
            last_value = self.lower_request(session, builder, blocks, block_id, step)?;
            if index + 1 < steps.len() {
                let next = step_blocks[index + 1];
                // Rewrite every Return produced by this step (including nested eval/bind
                // blocks from Immediate compound `Define`) into a jump to the next step.
                self.rewrite_returns_to_continue(builder, blocks, next)?;
            }
        }
        Ok(last_value)
    }

    /// Chain sequence steps: turn outstanding `Return` terminators into jumps to `next`.
    fn rewrite_returns_to_continue(&self, builder: &mut ModuleBuilder, blocks: &mut Vec<BasicBlock>, next: BlockId) -> Result<()> {
        let return_block_ids: Vec<BlockId> =
            blocks.iter().filter(|b| matches!(b.terminator, Terminator::Return { .. })).map(|b| b.id).collect();
        for block_id in return_block_ids {
            let cond = builder.ssa();
            let true_const = builder.push_constant(ConstantValue::boolean(true));
            let block = blocks.iter_mut().find(|b| b.id == block_id).expect("block");
            block.operations.push(Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: true_const },
                effect_in: None,
                effect_out: None,
            });
            block.terminator = Terminator::Branch { condition: cond, then_edge: BlockEdge::jump(next), else_edge: BlockEdge::jump(next) };
        }
        Ok(())
    }

    fn lower_branch(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        condition: TermId,
        then_branch: &AthenaRequest,
        else_branch: Option<&AthenaRequest>,
    ) -> Result<SsaValueId> {
        let then_block = builder.block_id();
        let else_block = builder.block_id();
        let mut operations = Vec::new();
        let cond_value = match self.require_boolean_atom(session, condition) {
            Ok(cond_bool) => {
                let cond_value = builder.ssa();
                let cond_constant = builder.push_constant(ConstantValue::boolean(cond_bool));
                operations.push(Operation {
                    result: Some(cond_value),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: cond_constant },
                    effect_in: None,
                    effect_out: None,
                });
                cond_value
            }
            Err(_) => {
                // Runtime predicate: `Equal[...]`, `True`/`False` symbols, numeric truthiness, etc.
                self.lower_pure_expr(session, builder, &mut operations, condition)?
            }
        };

        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations,
            terminator: Terminator::Branch {
                condition: cond_value,
                then_edge: BlockEdge::jump(then_block),
                else_edge: BlockEdge::jump(else_block),
            },
        });

        let then_value = self.lower_request(session, builder, blocks, then_block, then_branch)?;
        match else_branch {
            Some(request) => {
                let _else_value = self.lower_request(session, builder, blocks, else_block, request)?;
                Ok(then_value)
            }
            None => self.lower_unit_else(builder, blocks, else_block, then_value),
        }
    }

    fn lower_unit_else(
        &self,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        else_block: BlockId,
        then_value: SsaValueId,
    ) -> Result<SsaValueId> {
        // Missing else of `If`/`Branch` publishes as `Null` (Unit → Null at result materialization).
        let value = builder.ssa();
        let constant = builder.push_constant(ConstantValue::Unit);
        blocks.push(BasicBlock {
            id: else_block,
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(value),
                result_type: ExecutionValueType::Unit,
                kind: OperationKind::Constant { constant },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::return_value(value),
        });
        Ok(then_value)
    }

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

    /// Lower pure atom / Boolean semantic applications into SSA ops (no Session effects).
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
                match session.arena.symbols().resolve(*symbol) {
                    Some("True") => {
                        let ssa = builder.ssa();
                        let constant = builder.push_constant(ConstantValue::boolean(true));
                        operations.push(Operation {
                            result: Some(ssa),
                            result_type: ExecutionValueType::Boolean,
                            kind: OperationKind::Constant { constant },
                            effect_in: None,
                            effect_out: None,
                        });
                        return Ok(ssa);
                    }
                    Some("False") => {
                        let ssa = builder.ssa();
                        let constant = builder.push_constant(ConstantValue::boolean(false));
                        operations.push(Operation {
                            result: Some(ssa),
                            result_type: ExecutionValueType::Boolean,
                            kind: OperationKind::Constant { constant },
                            effect_in: None,
                            effect_out: None,
                        });
                        return Ok(ssa);
                    }
                    Some("Null") => {
                        let root = builder.push_term_root(term);
                        let ssa = builder.ssa();
                        operations.push(Operation {
                            result: Some(ssa),
                            result_type: ExecutionValueType::Term,
                            kind: OperationKind::LoadTerm { root },
                            effect_in: None,
                            effect_out: None,
                        });
                        return Ok(ssa);
                    }
                    _ => {}
                }
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
                let root = builder.push_term_root(term);
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
                let name = session.operators.name(head).ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "unknown_operator")
                })?;
                // Flatten left-nested compare chains before arg eval (`Less[Less[1,2],3]` → `Less[1,2,3]`).
                let compare_args = if matches!(name, "Less" | "Greater" | "LessEqual" | "GreaterEqual") {
                    flatten_compare_chain_args(session, name, term)
                }
                else {
                    None
                };
                let arg_terms = compare_args.unwrap_or(arguments);
                // Known semantic ops + unknown heads as Term residuals (CAS stays symbolic).
                let result_type = match name {
                    "Not" | "And" | "Or" | "TrueQ" | "SameQ" | "Equal" | "Unequal" | "Less" | "Greater" | "LessEqual" | "GreaterEqual" => {
                        ExecutionValueType::Boolean
                    }
                    _ => ExecutionValueType::Term,
                };
                // `Table` / iterator `Sum`/`Product`: HoldAll-ish body (first arg), evaluate iterator.
                // `CollectMatches` / `Matches`: HoldAll-ish pattern (second arg).
                // `Function`: HoldAll args (formal + body).
                let hold_all = name == "Function";
                let hold_first = matches!(name, "Table" | "Product") || (name == "Sum" && arg_terms.len() == 2);
                let hold_second = matches!(name, "CollectMatches" | "Matches") && arg_terms.len() >= 2;
                let mut args = Vec::with_capacity(arg_terms.len());
                for (index, arg) in arg_terms.into_iter().enumerate() {
                    if hold_all || (hold_first && index == 0) || (hold_second && index == 1) {
                        let root = builder.push_term_root(arg);
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
                    kind: OperationKind::ApplySemanticOperator { operator: head, args },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            Some(TermNode::List(items)) => {
                let items = items.clone();
                let mut elements = Vec::with_capacity(items.len());
                for item in items {
                    elements.push(self.lower_pure_expr(session, builder, operations, item)?);
                }
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::MakeList { elements },
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
            Some(TermNode::Atom(Atom::Symbol(symbol))) => match session.arena.symbols().resolve(*symbol) {
                Some("True") => Ok(true),
                Some("False") => Ok(false),
                _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionCompiler")
                    .detail("status", "branch_condition_not_boolean_atom")),
            },
            // Exact `0`/`1` truthiness. Other numbers fail so
            // `Branch` can fall back to runtime predicate lowering.
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


#[cfg(test)]
mod tests;
