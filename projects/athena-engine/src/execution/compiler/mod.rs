//! `ExecutionCompiler` — `AthenaRequest` + Session snapshot → [`ExecutionModule`].
//!
//! Bootstrap lowering: atom terms, typed Boolean constants, `ControlPlan::Branch` /
//! `Sequence`, and effectful `SessionCommand::Define` via `WriteBinding`.
//! No bridge to the old stack VM / `KernelIR` path.

use athena_ir::{Atom, TermNode};
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

use crate::api::request::{AthenaRequest, ControlPlan, DefinitionEvaluationTiming, SessionCommand};
use crate::execution::ir::{
    BasicBlock, BlockEdge, BlockId, CapturedRoot, CapturedRootId, ConstantId, ConstantValue, EffectEdge, EffectKind,
    EffectToken, ExecutionModule, ExecutionValueType, ModuleFingerprint, Operation, OperationKind, Region, RegionId,
    SsaValueId, Terminator, verify_module,
};
use crate::runtime::session::Session;

/// Compiles one request into a verified [`ExecutionModule`].
#[derive(Debug, Default)]
pub struct ExecutionCompiler {}

#[derive(Default)]
struct ModuleBuilder {
    constants: Vec<ConstantValue>,
    captured_roots: Vec<CapturedRoot>,
    effect_edges: Vec<EffectEdge>,
    next_ssa: u32,
    next_block: u32,
    next_effect: u32,
}

impl ModuleBuilder {
    fn ssa(&mut self) -> SsaValueId {
        let id = SsaValueId(self.next_ssa);
        self.next_ssa = self.next_ssa.saturating_add(1);
        id
    }

    fn block_id(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block = self.next_block.saturating_add(1);
        id
    }

    fn push_constant(&mut self, value: ConstantValue) -> ConstantId {
        let id = ConstantId(self.constants.len() as u32);
        self.constants.push(value);
        id
    }

    fn push_term_root(&mut self, term: TermId) -> CapturedRootId {
        let id = CapturedRootId(self.captured_roots.len() as u32);
        self.captured_roots.push(CapturedRoot::term(term));
        id
    }

    fn push_effect(&mut self, kind: EffectKind, precedes_from: Option<EffectToken>) -> EffectToken {
        let token = EffectToken(self.next_effect);
        self.next_effect = self.next_effect.saturating_add(1);
        self.effect_edges.push(match precedes_from {
            Some(prev) => EffectEdge::after(token, prev, kind),
            None => EffectEdge::entry(token, kind),
        });
        token
    }

    fn finish(self, blocks: Vec<BasicBlock>, entry: BlockId) -> Result<ExecutionModule> {
        let region = Region {
            id: RegionId(0),
            entry,
            blocks,
            result_types: vec![ExecutionValueType::Term],
        };
        let mut module = ExecutionModule {
            inputs: Vec::new(),
            constants: self.constants,
            captured_roots: self.captured_roots,
            regions: vec![region],
            effect_edges: self.effect_edges,
            exits: Vec::new(),
            provider_calls: Vec::new(),
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        verify_module(&module)?;
        Ok(module)
    }
}

impl ExecutionCompiler {
    /// Create a compiler instance.
    pub fn new() -> Self {
        Self {}
    }

    /// Lower a request against a Session snapshot into `ExecutionIR`.
    pub fn compile(&self, session: &Session, request: &AthenaRequest) -> Result<ExecutionModule> {
        let mut builder = ModuleBuilder::default();
        let entry = builder.block_id();
        let mut blocks = Vec::new();
        let value = self.lower_request(session, &mut builder, &mut blocks, entry, request)?;
        // Ensure entry block exists and returns when lowering produced a single block return.
        if blocks.iter().all(|b| b.id != entry) {
            blocks.insert(
                0,
                BasicBlock {
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::return_value(value),
                },
            );
        }
        builder.finish(blocks, entry)
    }

    fn lower_request(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        request: &AthenaRequest,
    ) -> Result<SsaValueId> {
        match request {
            AthenaRequest::Term(term) => self.lower_term_into_block(session, builder, blocks, block_id, *term),
            AthenaRequest::Control(plan) => self.lower_control(session, builder, blocks, block_id, plan),
            AthenaRequest::Command(command) => self.lower_command(session, builder, blocks, block_id, command),
            AthenaRequest::Goal(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "request_kind_not_lowered")
                .detail("kind", request.kind_name())),
        }
    }

    fn lower_command(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        command: &SessionCommand,
    ) -> Result<SsaValueId> {
        match command {
            SessionCommand::Define {
                symbol,
                value,
                timing: DefinitionEvaluationTiming::Immediate | DefinitionEvaluationTiming::Deferred,
            } => {
                // Atom rhs only: Immediate and Deferred coincide for already-normalized atoms.
                self.require_atom(session, *value)?;
                let key = builder.ssa();
                let key_constant = builder.push_constant(ConstantValue::symbol(*symbol));
                let root = builder.push_term_root(*value);
                let rhs = builder.ssa();
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
                            result: Some(rhs),
                            result_type: ExecutionValueType::Term,
                            kind: OperationKind::LoadTerm { root },
                            effect_in: None,
                            effect_out: None,
                        },
                        Operation {
                            result: Some(unit),
                            result_type: ExecutionValueType::Unit,
                            kind: OperationKind::WriteBinding { key, value: rhs },
                            effect_in: Some(effect_in),
                            effect_out: Some(effect_out),
                        },
                    ],
                    terminator: Terminator::return_value(unit),
                });
                Ok(unit)
            }
            SessionCommand::ClearDefinition { .. } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "clear_definition_not_lowered")),
        }
    }

    fn lower_control(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        plan: &ControlPlan,
    ) -> Result<SsaValueId> {
        match plan {
            ControlPlan::Sequence { steps } => {
                if steps.is_empty() {
                    let value = builder.ssa();
                    let constant = builder.push_constant(ConstantValue::Unit);
                    blocks.push(BasicBlock {
                        id: block_id,
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
                // Bootstrap: only the last step is material; earlier steps must be pure atoms
                // already represented as Term requests (no Session effects yet).
                for step in &steps[..steps.len() - 1] {
                    match step {
                        AthenaRequest::Term(term) => {
                            self.require_atom(session, *term)?;
                        }
                        _ => {
                            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                                .detail("component", "ExecutionCompiler")
                                .detail("status", "sequence_step_not_lowered"));
                        }
                    }
                }
                self.lower_request(session, builder, blocks, block_id, steps.last().expect("non-empty"))
            }
            ControlPlan::Branch {
                condition,
                then_branch,
                else_branch,
            } => self.lower_branch(session, builder, blocks, block_id, *condition, then_branch, else_branch.as_deref()),
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "control_plan_not_lowered")),
        }
    }

    fn lower_branch(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        condition: TermId,
        then_branch: &AthenaRequest,
        else_branch: Option<&AthenaRequest>,
    ) -> Result<SsaValueId> {
        let cond_bool = self.require_boolean_atom(session, condition)?;
        let cond_value = builder.ssa();
        let cond_constant = builder.push_constant(ConstantValue::boolean(cond_bool));
        let then_block = builder.block_id();
        let else_block = builder.block_id();

        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(cond_value),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: cond_constant },
                effect_in: None,
                effect_out: None,
            }],
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
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        term: TermId,
    ) -> Result<SsaValueId> {
        let mut operations = Vec::new();
        let value = self.lower_pure_expr(session, builder, &mut operations, term)?;
        blocks.push(BasicBlock {
            id: block_id,
            parameters: Vec::new(),
            operations,
            terminator: Terminator::return_value(value),
        });
        Ok(value)
    }

    /// Lower pure atom / Boolean semantic applications into SSA ops (no Session effects).
    fn lower_pure_expr(
        &self,
        session: &Session,
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
                let name = session.operators.name(*head).ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "unknown_operator")
                })?;
                if !matches!(name, "Not" | "And" | "Or") {
                    return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "compound_term_not_lowered")
                        .detail("operator", name));
                }
                let mut args = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    args.push(self.lower_pure_expr(session, builder, operations, *arg)?);
                }
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::ApplySemanticOperator {
                        operator: *head,
                        args,
                    },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            Some(TermNode::List(_)) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "list_term_not_lowered")),
            None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("component", "ExecutionCompiler")
                .detail("reason", "missing_term")),
        }
    }

    fn require_atom(&self, session: &Session, term: TermId) -> Result<()> {
        match session.arena.get(term) {
            Some(TermNode::Atom(_)) => Ok(()),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "compound_term_not_lowered")),
            None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("component", "ExecutionCompiler")
                .detail("reason", "missing_term")),
        }
    }

    fn require_boolean_atom(&self, session: &Session, term: TermId) -> Result<bool> {
        match session.arena.get(term) {
            Some(TermNode::Atom(Atom::Boolean(value))) => Ok(*value),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "branch_condition_not_boolean_atom")),
            None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("component", "ExecutionCompiler")
                .detail("reason", "missing_term")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::reference::ReferenceExecutor;
    use crate::runtime::session::Session;
    use athena_types::ComputationStatus;

    #[test]
    fn compile_atom_term_module() {
        let mut session = Session::new();
        let term = session.builder().int(3, Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("atom");
        assert_eq!(module.captured_roots, vec![CapturedRoot::term(term)]);
        assert_eq!(module.regions.len(), 1);
    }

    #[test]
    fn compile_application_rejected() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", Default::default());
        let plus = session.operators.intern("Plus");
        let term = session.builder().application(plus, vec![x, x], Default::default());
        let err = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect_err("compound");
        assert_eq!(err.code, DiagnosticCode::UnsupportedOperation);
    }

    #[test]
    fn compile_and_execute_boolean_branch() {
        let mut session = Session::new();
        let cond = session.builder().boolean(true, Default::default());
        let then_term = session.builder().int(1, Default::default());
        let else_term = session.builder().int(0, Default::default());
        let request = AthenaRequest::Control(ControlPlan::Branch {
            condition: cond,
            then_branch: Box::new(AthenaRequest::Term(then_term)),
            else_branch: Some(Box::new(AthenaRequest::Term(else_term))),
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("branch");
        assert_eq!(module.regions[0].blocks.len(), 3);
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(then_term));
        assert_eq!(loaded.status, ComputationStatus::Exact);
    }

    #[test]
    fn compile_and_execute_define_write_binding() {
        use crate::api::request::{DefinitionEvaluationTiming, SessionCommand};

        let mut session = Session::new();
        let sym_term = session.builder().symbol("x", Default::default());
        let symbol = match session.arena.get(sym_term) {
            Some(TermNode::Atom(Atom::Symbol(id))) => *id,
            other => panic!("expected symbol atom, got {other:?}"),
        };
        let value = session.builder().int(42, Default::default());
        let request = AthenaRequest::Command(SessionCommand::Define {
            symbol,
            value,
            timing: DefinitionEvaluationTiming::Immediate,
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("define");
        assert!(!module.effect_edges.is_empty());
        ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
        assert_eq!(session.defs.own(symbol), Some(value));
    }

    #[test]
    fn compile_and_execute_define_then_read_binding() {
        use crate::api::request::{DefinitionEvaluationTiming, SessionCommand};

        let mut session = Session::new();
        let sym_term = session.builder().symbol("y", Default::default());
        let symbol = match session.arena.get(sym_term) {
            Some(TermNode::Atom(Atom::Symbol(id))) => *id,
            other => panic!("expected symbol atom, got {other:?}"),
        };
        let value = session.builder().int(7, Default::default());
        let define = AthenaRequest::Command(SessionCommand::Define {
            symbol,
            value,
            timing: DefinitionEvaluationTiming::Immediate,
        });
        let define_module = ExecutionCompiler::new().compile(&session, &define).expect("define");
        ReferenceExecutor::new().execute(&mut session, &define_module).expect("define exec");

        let read = AthenaRequest::Term(sym_term);
        let read_module = ExecutionCompiler::new().compile(&session, &read).expect("read");
        let result_id = ReferenceExecutor::new().execute(&mut session, &read_module).expect("read exec");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(value));
    }

    #[test]
    fn compile_and_execute_boolean_not_and() {
        let mut session = Session::new();
        let t = session.builder().boolean(true, Default::default());
        let f = session.builder().boolean(false, Default::default());
        let and = session.operators.intern("And");
        let not = session.operators.intern("Not");
        let and_term = session.builder().application(and, vec![t, f], Default::default());
        let term = session.builder().application(not, vec![and_term], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("bool ops");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::Atom(Atom::Boolean(true))) => {}
            other => panic!("expected Not[And[True,False]] == True, got {other:?}"),
        }
    }
}
