//! `ExecutionCompiler` — `AthenaRequest` + Session snapshot → [`ExecutionModule`].
//!
//! Bootstrap lowering: atom terms, typed Boolean constants, `ControlPlan::Branch` /
//! `Sequence`, and effectful `SessionCommand::Define` via `WriteBinding`.
//! No bridge to a deleted stack interpreter.

use athena_ir::{Atom, TermNode};
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

/// Compiles one request into a verified [`ExecutionModule`].
#[derive(Debug, Default)]
pub struct ExecutionCompiler {}


mod builder;
mod helpers;
mod control;
mod define;

use builder::ModuleBuilder;
use helpers::{collect_compare_chain_args, flatten_compare_chain_args};

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
                                &SessionCommand::Define {
                                    symbol,
                                    value: *rhs,
                                    kind: BindingKind::Session,
                                    evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
                                },
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
                            &SessionCommand::Define {
                                symbol: *symbol,
                                value: *rhs,
                                kind: BindingKind::Session,
                                evaluation: BindingEvaluationPolicy::StoreResidualTerm,
                            },
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
                                self.lower_register_rule(session, builder, blocks, block_id, symbol, *lhs, *rhs)
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
}

impl ExecutionCompiler {
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
            SessionCommand::Define { symbol, value, kind: _, evaluation } => {
                let residual = !matches!(evaluation, BindingEvaluationPolicy::EvaluateBeforeStore);
                if residual {
                    return self.lower_define_capture(session, builder, blocks, block_id, *symbol, *value, *evaluation);
                }
                match session.arena.get(*value) {
                    Some(TermNode::Atom(_)) => self.lower_define_capture(session, builder, blocks, block_id, *symbol, *value, BindingEvaluationPolicy::EvaluateBeforeStore),
                    Some(_) => self.lower_define_evaluated(session, builder, blocks, block_id, *symbol, *value),
                    None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                        .detail("component", "ExecutionCompiler")
                        .detail("reason", "missing_term")),
                }
            }
            SessionCommand::RegisterRuleDispatch { .. } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "register_rule_dispatch_pending_opcode")),
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
