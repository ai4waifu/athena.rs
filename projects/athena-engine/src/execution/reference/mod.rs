//! `ReferenceExecutor` — correctness / replay backend for [`ExecutionModule`].
//!
//! Executes SSA blocks without an operand stack. Not a wrapper around the old VM.

use std::collections::HashMap;

use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode, Result, ResultId, SymbolId, TermId};

use crate::execution::ir::{
    BlockId, CapturedRoot, ConstantValue, ExecutionModule, OperationKind, RegionId, SsaValueId, Terminator, verify_module,
};
use crate::runtime::results::{ComputationResult, CoverageStatus, ResultProvenance};
use crate::runtime::session::Session;

/// Semantic oracle backend shared by parity tests and deterministic replay.
#[derive(Debug, Default)]
pub struct ReferenceExecutor {}

/// SSA runtime slot (session-local; not an identity domain shared with `TermId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    /// Term handle from `TermStore`.
    Term(TermId),
    /// Binding key.
    Symbol(SymbolId),
    /// Typed Boolean.
    Boolean(bool),
    /// Unit.
    Unit,
}

impl ReferenceExecutor {
    /// Create a reference executor.
    pub fn new() -> Self {
        Self {}
    }

    /// Execute a verified module in the given Session / runtime context.
    pub fn execute(&self, session: &mut Session, module: &ExecutionModule) -> Result<ResultId> {
        verify_module(module)?;
        let region_id = module.entry_region().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ReferenceExecutor")
                .detail("reason", "missing_entry_region")
        })?;
        let returned = self.eval_region(session, module, region_id)?;
        let term = match returned {
            Some(Slot::Term(term)) => term,
            Some(Slot::Boolean(value)) => session.builder().boolean(value, Default::default()),
            Some(Slot::Symbol(symbol)) => session.builder().symbol_id(symbol, Default::default()),
            Some(Slot::Unit) | None => session.builder().null(Default::default()),
        };
        let value = session.insert_symbolic_value(term);
        let result = ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full)
            .with_value(value)
            .with_symbolic_term(term)
            .with_provenance(ResultProvenance { request_kind: "ExecutionIR" });
        Ok(session.insert_result(result))
    }

    fn eval_region(&self, session: &mut Session, module: &ExecutionModule, region_id: RegionId) -> Result<Option<Slot>> {
        let region = module
            .regions
            .iter()
            .find(|r| r.id == region_id)
            .ok_or_else(|| diag("missing_region"))?;
        let mut block_id = region.entry;
        let mut slots: HashMap<SsaValueId, Slot> = HashMap::new();
        // Bootstrap: linear / single-block regions and simple branches only.
        for _ in 0..region.blocks.len().saturating_mul(4).max(4) {
            let block = region
                .blocks
                .iter()
                .find(|b| b.id == block_id)
                .ok_or_else(|| diag("missing_block"))?;
            for op in &block.operations {
                let produced = self.eval_operation(session, module, &slots, &op.kind)?;
                if let Some(result) = op.result {
                    slots.insert(result, produced);
                }
            }
            match &block.terminator {
                Terminator::Return { values } => {
                    if values.is_empty() {
                        return Ok(Some(Slot::Unit));
                    }
                    let first = values[0];
                    return Ok(Some(*slots.get(&first).ok_or_else(|| diag("return_undefined"))?));
                }
                Terminator::Branch { condition, then_edge, else_edge } => {
                    let pred = match slots.get(condition).ok_or_else(|| diag("branch_undefined"))? {
                        Slot::Boolean(v) => *v,
                        _ => return Err(diag("branch_not_boolean")),
                    };
                    let edge = if pred { then_edge } else { else_edge };
                    self.bind_edge_args(&mut slots, region, edge.target, &edge.arguments)?;
                    block_id = edge.target;
                }
                Terminator::Reject { .. } => {
                    return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ReferenceExecutor")
                        .detail("reason", "rejected"));
                }
                Terminator::Unreachable => return Err(diag("unreachable")),
                Terminator::Switch { .. } | Terminator::Yield { .. } => {
                    return Err(diag("terminator_not_implemented"));
                }
            }
        }
        Err(diag("block_visit_budget_exceeded"))
    }

    fn bind_edge_args(
        &self,
        slots: &mut HashMap<SsaValueId, Slot>,
        region: &crate::execution::ir::Region,
        target: BlockId,
        arguments: &[SsaValueId],
    ) -> Result<()> {
        let block = region.blocks.iter().find(|b| b.id == target).ok_or_else(|| diag("edge_target_missing"))?;
        if block.parameters.len() != arguments.len() {
            return Err(diag("edge_arity_mismatch"));
        }
        for (param, arg) in block.parameters.iter().zip(arguments.iter()) {
            let value = *slots.get(arg).ok_or_else(|| diag("edge_arg_undefined"))?;
            slots.insert(param.value, value);
        }
        Ok(())
    }

    fn eval_operation(
        &self,
        session: &mut Session,
        module: &ExecutionModule,
        slots: &HashMap<SsaValueId, Slot>,
        kind: &OperationKind,
    ) -> Result<Slot> {
        match kind {
            OperationKind::Constant { constant } => {
                let value = module.constants.get(constant.0 as usize).ok_or_else(|| diag("missing_constant"))?;
                Ok(match value {
                    ConstantValue::Boolean(v) => Slot::Boolean(*v),
                    ConstantValue::Symbol(symbol) => Slot::Symbol(*symbol),
                    ConstantValue::Term(term) => Slot::Term(*term),
                    ConstantValue::Unit => Slot::Unit,
                })
            }
            OperationKind::LoadTerm { root } => {
                let captured = module.captured_roots.get(root.0 as usize).ok_or_else(|| diag("missing_root"))?;
                match captured {
                    CapturedRoot::Term(term) => Ok(Slot::Term(*term)),
                    CapturedRoot::Value(_) | CapturedRoot::Result(_) => Err(diag("root_not_term")),
                }
            }
            OperationKind::ApplySemanticOperator { operator, args } => {
                let name = session.operators.name(*operator).ok_or_else(|| diag("unknown_operator"))?;
                let bools = args
                    .iter()
                    .map(|id| match slots.get(id) {
                        Some(Slot::Boolean(v)) => Ok(*v),
                        _ => Err(diag("semantic_arg_not_boolean")),
                    })
                    .collect::<Result<Vec<_>>>()?;
                let result = match (name, bools.as_slice()) {
                    ("Not", [a]) => !*a,
                    ("And", values) => values.iter().copied().all(|v| v),
                    ("Or", values) => values.iter().copied().any(|v| v),
                    _ => return Err(diag("semantic_operator_not_implemented")),
                };
                Ok(Slot::Boolean(result))
            }
            OperationKind::WriteBinding { key, value } => {
                let symbol = match slots.get(key) {
                    Some(Slot::Symbol(symbol)) => *symbol,
                    _ => return Err(diag("write_key_not_symbol")),
                };
                match slots.get(value) {
                    Some(Slot::Unit) => {
                        session.defs.clear_symbol(symbol);
                    }
                    Some(Slot::Term(term)) => {
                        session.defs.define_own(symbol, *term);
                    }
                    Some(Slot::Boolean(v)) => {
                        let term = session.builder().boolean(*v, Default::default());
                        session.defs.define_own(symbol, term);
                    }
                    _ => return Err(diag("write_value_unsupported")),
                }
                Ok(Slot::Unit)
            }
            OperationKind::ReadBinding { key } => {
                let symbol = match slots.get(key) {
                    Some(Slot::Symbol(symbol)) => *symbol,
                    _ => return Err(diag("read_key_not_symbol")),
                };
                if let Some(term) = session.defs.own(symbol) {
                    return Ok(Slot::Term(term));
                }
                if let Some(term) = session.defs.delayed(symbol) {
                    return Ok(Slot::Term(term));
                }
                // Unbound symbol evaluates to itself (residual Term).
                Ok(Slot::Term(session.builder().symbol_id(symbol, Default::default())))
            }
            OperationKind::LoadInput { .. }
            | OperationKind::EnterScope { .. }
            | OperationKind::ExitScope { .. }
            | OperationKind::CallProvider { .. }
            | OperationKind::Guard { .. }
            | OperationKind::MaterializeValue { .. }
            | OperationKind::PublishResult { .. } => Err(diag("operation_not_implemented")),
        }
    }
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("component", "ReferenceExecutor")
        .detail("reason", reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::request::AthenaRequest;
    use crate::execution::compiler::ExecutionCompiler;
    use crate::execution::ir::{
        BasicBlock, BlockEdge, BlockId, ConstantId, ConstantValue, ExecutionValueType, ModuleFingerprint, Operation, Region,
        RegionId, Terminator,
    };

    #[test]
    fn execute_compiled_atom_term() {
        let mut session = Session::new();
        let term = session.builder().int(9, Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("compile");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(term));
        assert_eq!(loaded.status, ComputationStatus::Exact);
        assert_eq!(loaded.coverage, CoverageStatus::Full);
    }

    #[test]
    fn execute_boolean_branch() {
        let cond = SsaValueId(0);
        let then_v = SsaValueId(1);
        let else_v = SsaValueId(2);
        let entry = BasicBlock {
            id: BlockId(0),
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant {
                    constant: ConstantId(0),
                },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::Branch {
                condition: cond,
                then_edge: BlockEdge::jump(BlockId(1)),
                else_edge: BlockEdge::jump(BlockId(2)),
            },
        };
        let then_block = BasicBlock {
            id: BlockId(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(then_v),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant {
                    constant: ConstantId(1),
                },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::return_value(then_v),
        };
        let else_block = BasicBlock {
            id: BlockId(2),
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(else_v),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant {
                    constant: ConstantId(2),
                },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::return_value(else_v),
        };
        let region = Region {
            id: RegionId(0),
            entry: BlockId(0),
            blocks: vec![entry, then_block, else_block],
            result_types: vec![ExecutionValueType::Boolean],
        };
        let mut module = ExecutionModule {
            inputs: Vec::new(),
            constants: vec![ConstantValue::boolean(true), ConstantValue::boolean(true), ConstantValue::boolean(false)],
            captured_roots: Vec::new(),
            regions: vec![region],
            effect_edges: Vec::new(),
            exits: Vec::new(),
            provider_calls: Vec::new(),
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);

        let mut session = Session::new();
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("branch");
        let loaded = session.results.get(result_id).expect("result");
        let term = loaded.symbolic_term.expect("term");
        match session.arena.get(term) {
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(true))) => {}
            other => panic!("expected true boolean term, got {other:?}"),
        }
    }
}
