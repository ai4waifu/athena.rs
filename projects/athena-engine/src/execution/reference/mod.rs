//! `ReferenceExecutor` — correctness / replay backend for [`ExecutionModule`].
//!
//! Executes SSA blocks without an operand stack. Not a wrapper around the old VM.

mod helpers;
mod ops;

use self::helpers::*;

use std::{cmp::Ordering, collections::HashMap};

use athena_numeric::{
    Integer, Number, Rational, abs as num_abs, add as num_add, compare as num_compare, div as num_div, factorial as num_factorial,
    mul as num_mul, pow as num_pow, sqrt as num_sqrt, to_f64_lossy as num_to_f64_lossy,
};
use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode, Result, ResultId, SymbolId, TermId};

use crate::{
    api::request::AthenaRequest,
    domains::{
        calculus::{CalculusCtx, execute_calculus, materialize_calculus_result_term, try_calculus_request},
        dispatch::{DomainRequest, execute_domain},
        linear_algebra::{MatrixEntry, MatrixValue, SolveDisposition, det_bareiss, solve_exact},
    },
    execution::{
        compiler::ExecutionCompiler,
        environment::{LocalBinding, ScopeFrame},
        ir::{BlockId, CapturedRoot, ConstantValue, ExecutionModule, OperationKind, RegionId, SsaValueId, Terminator, verify_module},
        number_of, push_application, push_number,
    },
    runtime::{
        results::{ComputationResult, CoverageStatus, ResultProvenance, computation_from_domain},
        session::Session,
        values::{
            arena::push_list,
            numeric_clone::{clone_integer, clone_number, clone_rational},
        },
    },
};

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
    /// Scope frame depth handle from `EnterScope`.
    Scope(u32),
    /// Already-materialized `ComputationResult` (domain provider).
    Result(ResultId),
    /// Unit.
    Unit,
}

#[derive(Debug)]
enum PartStep {
    Next(TermId),
    Residual,
    Invalid { echo: TermId, diagnostic: Diagnostic },
}

impl ReferenceExecutor {
    /// Create a reference executor.
    pub fn new() -> Self {
        Self {}
    }

    /// Execute a verified module in the given Session / runtime context.
    ///
    /// When `domain` is `Some`, the first `CallProvider` edge runs `execute_domain`
    /// and returns that materialized `ResultId` (IR-shaped Goal path).
    pub fn execute(&self, session: &mut Session, module: &ExecutionModule, domain: Option<DomainRequest>) -> Result<ResultId> {
        verify_module(module)?;
        let region_id = module.entry_region().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ReferenceExecutor")
                .detail("reason", "missing_entry_region")
        })?;
        let mut provider = domain;
        let (returned, unsupported, unevaluated, invalid) = self.eval_region(session, module, region_id, &mut provider)?;
        if let Some(Slot::Result(result_id)) = returned {
            return Ok(result_id);
        }
        let term = match returned {
            Some(Slot::Term(term)) => term,
            Some(Slot::Boolean(value)) => session.builder().boolean(value, Default::default()),
            Some(Slot::Symbol(symbol)) => session.builder().symbol_id(symbol, Default::default()),
            Some(Slot::Scope(_)) | Some(Slot::Unit) | Some(Slot::Result(_)) | None => session.builder().null(Default::default()),
        };
        let value = session.insert_symbolic_value(term);
        let mut result = if let Some(diagnostic) = invalid {
            ComputationResult::with_status(ComputationStatus::Invalid, CoverageStatus::Partial).with_diagnostic(diagnostic)
        }
        else if unsupported {
            ComputationResult::with_status(ComputationStatus::Unknown, CoverageStatus::Unsupported).with_diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "CallProvider")
                    .detail("status", "provider_bootstrap_unsupported"),
            )
        }
        else if unevaluated {
            ComputationResult::with_status(ComputationStatus::Unknown, CoverageStatus::Partial)
        }
        else {
            ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full)
        };
        result = result.with_value(value).with_symbolic_term(term).with_provenance(ResultProvenance::kind("ExecutionIR"));
        Ok(session.insert_result(result))
    }

    fn eval_region(
        &self,
        session: &mut Session,
        module: &ExecutionModule,
        region_id: RegionId,
        provider: &mut Option<DomainRequest>,
    ) -> Result<(Option<Slot>, bool, bool, Option<Diagnostic>)> {
        let region = module.regions.iter().find(|r| r.id == region_id).ok_or_else(|| diag("missing_region"))?;
        let mut block_id = region.entry;
        let mut slots: HashMap<SsaValueId, Slot> = HashMap::new();
        let mut frames: Vec<ScopeFrame> = Vec::new();
        let mut unsupported = false;
        let mut unevaluated = false;
        let mut invalid: Option<Diagnostic> = None;
        let mut block_visits: HashMap<BlockId, u32> = HashMap::new();
        // Bootstrap: allow limited loop back-edges; cap per-block visits.
        for _ in 0..region.blocks.len().saturating_mul(64).max(64) {
            let visits = block_visits.entry(block_id).or_insert(0);
            *visits = visits.saturating_add(1);
            if *visits > 32 {
                // Budget exhausted on a hot block — exit with Unit residual.
                return Ok((Some(Slot::Unit), unsupported, unevaluated, invalid));
            }
            let block = region.blocks.iter().find(|b| b.id == block_id).ok_or_else(|| diag("missing_block"))?;
            for op in &block.operations {
                let produced = self.eval_operation(
                    session,
                    module,
                    &slots,
                    &mut frames,
                    &mut unsupported,
                    &mut unevaluated,
                    &mut invalid,
                    provider,
                    &op.kind,
                )?;
                if let Some(result) = op.result {
                    slots.insert(result, produced);
                }
            }
            match &block.terminator {
                Terminator::Return { values } => {
                    if values.is_empty() {
                        return Ok((Some(Slot::Unit), unsupported, unevaluated, invalid));
                    }
                    let first = values[0];
                    return Ok((Some(*slots.get(&first).ok_or_else(|| diag("return_undefined"))?), unsupported, unevaluated, invalid));
                }
                Terminator::Branch { condition, then_edge, else_edge } => {
                    let pred = match slots.get(condition).ok_or_else(|| diag("branch_undefined"))? {
                        Slot::Boolean(v) => Ok(*v),
                        Slot::Term(term) => coerce_branch_predicate(session, *term),
                        _ => Err(Diagnostic::new(DiagnosticCode::NonBooleanCondition)
                            .detail("component", "ReferenceExecutor")
                            .detail("reason", "branch_not_boolean")),
                    };
                    let pred = match pred {
                        Ok(v) => v,
                        Err(diagnostic) => {
                            // Soft-fail like VM: Invalid + unevaluated (Null residual).
                            return Ok((Some(Slot::Unit), unsupported, true, Some(diagnostic)));
                        }
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
        frames: &mut Vec<ScopeFrame>,
        unsupported: &mut bool,
        unevaluated: &mut bool,
        invalid: &mut Option<Diagnostic>,
        provider: &mut Option<DomainRequest>,
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
                let name = session.operators.name(*operator).ok_or_else(|| diag("unknown_operator"))?.to_string();
                match name.as_str() {
                    "Not" | "And" | "Or" | "TrueQ" => {
                        let bools: Vec<Option<bool>> = args
                            .iter()
                            .map(|id| {
                                let slot = slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
                                Ok(slot_as_boolean_like(session, *slot))
                            })
                            .collect::<Result<Vec<_>>>()?;
                        if bools.iter().any(|b| b.is_none()) {
                            // Non-boolean-like args stay as residual logic forms (VM parity).
                            return self.eval_residual_app(session, name.as_str(), args, slots);
                        }
                        let bools: Vec<bool> = bools.into_iter().map(|b| b.expect("checked")).collect();
                        let result = match (name.as_str(), bools.as_slice()) {
                            ("Not", [a]) => !*a,
                            ("TrueQ", [a]) => *a,
                            ("And", values) => values.iter().copied().all(|v| v),
                            ("Or", values) => values.iter().copied().any(|v| v),
                            _ => return Err(diag("semantic_operator_arity")),
                        };
                        Ok(Slot::Boolean(result))
                    }
                    "SameQ" | "Equal" | "Unequal" => {
                        if args.len() != 2 {
                            return Err(diag("semantic_operator_arity"));
                        }
                        let left = slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?;
                        let right = slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?;
                        let same = match (left, right) {
                            (Slot::Boolean(a), Slot::Boolean(b)) => a == b,
                            (Slot::Symbol(a), Slot::Symbol(b)) => a == b,
                            (Slot::Term(a), Slot::Term(b)) => session.arena.structural_eq(*a, *b),
                            (Slot::Unit, Slot::Unit) => true,
                            _ => false,
                        };
                        Ok(Slot::Boolean(if name == "Unequal" { !same } else { same }))
                    }
                    "Less" | "Greater" | "LessEqual" | "GreaterEqual" => self.eval_compare_chain(session, name.as_str(), args, slots),
                    "Plus" | "Times" | "Subtract" | "Divide" | "Power" => self.eval_arithmetic(session, name.as_str(), args, slots),
                    "Abs" | "Length" | "First" | "Rest" | "Factorial" | "Sqrt" => self.eval_unary_term_op(session, name.as_str(), args, slots),
                    "Join" => self.eval_join(session, args, slots),
                    "Part" => self.eval_part(session, args, slots, invalid),
                    "Span" => self.eval_span(session, args, slots),
                    "Range" => self.eval_range(session, args, slots),
                    "Apply" => self.eval_apply(session, args, slots),
                    "Application" => self.eval_application_form(session, args, slots),
                    "Size" => self.eval_size(session, args, slots),
                    "Sum" => self.eval_sum(session, args, slots),
                    "Table" => self.eval_table(session, args, slots),
                    "Det" => self.eval_det(session, args, slots, invalid),
                    "LinearSolve" => self.eval_linear_solve(session, args, slots, invalid),
                    "Map" => self.eval_map(session, args, slots),
                    "Zeros" | "Ones" | "Eye" => self.eval_matrix_constructor(session, name.as_str(), args, slots),
                    "Rule" | "RuleDeferred" => self.eval_rule(session, name.as_str(), args, slots),
                    "ReplaceAll" => self.eval_replace_all(session, args, slots),
                    "CollectMatches" => self.eval_collect_matches(session, args, slots),
                    "Matches" => self.eval_matches(session, args, slots),
                    "Simplify" => self.eval_simplify(session, args, slots),
                    "D" | "Integrate" | "Limit" | "Series" | "LaurentSeries" | "Asymptotic" | "Residue" | "DSolve" | "LaplaceTransform"
                    | "FourierTransform" | "ZTransform" | "Divergence" | "Curl" => self.eval_calculus(session, name.as_str(), args, slots),
                    "Import" | "Export" | "Timing" => {
                        *invalid = Some(
                            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                                .detail("component", "ReferenceExecutor")
                                .detail("operation", name.as_str()),
                        );
                        self.eval_residual_app(session, name.as_str(), args, slots)
                    }
                    _ => {
                        if let Some(slot) = self.try_apply_down_values(session, name.as_str(), args, slots)? {
                            return Ok(slot);
                        }
                        if !is_known_residual_head(name.as_str()) {
                            *unevaluated = true;
                        }
                        self.eval_residual_app(session, name.as_str(), args, slots)
                    }
                }
            }
            OperationKind::MakeList { elements } => {
                let mut items = Vec::with_capacity(elements.len());
                for id in elements {
                    let slot = *slots.get(id).ok_or_else(|| diag("list_element_undefined"))?;
                    items.push(self.slot_as_term(session, slot)?);
                }
                Ok(Slot::Term(push_list(session, items)))
            }
            OperationKind::EnterScope { .. } => {
                let depth = frames.len() as u32;
                frames.push(ScopeFrame::new());
                Ok(Slot::Scope(depth))
            }
            OperationKind::ExitScope { scope } => {
                let expected = match slots.get(scope) {
                    Some(Slot::Scope(depth)) => *depth,
                    _ => return Err(diag("exit_scope_bad_handle")),
                };
                let top = frames.len().saturating_sub(1) as u32;
                if expected != top {
                    return Err(diag("exit_scope_mismatch"));
                }
                frames.pop();
                Ok(Slot::Unit)
            }
            OperationKind::WriteBinding { key, value, delayed } => {
                let symbol = match slots.get(key) {
                    Some(Slot::Symbol(symbol)) => *symbol,
                    _ => return Err(diag("write_key_not_symbol")),
                };
                match slots.get(value) {
                    Some(Slot::Unit) => {
                        if let Some(frame) = frames.last_mut() {
                            frame.unbind(symbol);
                        }
                        else {
                            session.defs.clear_symbol(symbol);
                        }
                    }
                    Some(Slot::Term(term)) => {
                        if *delayed {
                            if let Some(frame) = frames.last_mut() {
                                // Bootstrap: local delayed stored as Own of captured rhs.
                                frame.bind(symbol, LocalBinding::Own(*term));
                            }
                            else {
                                session.defs.define_delayed(symbol, *term);
                            }
                        }
                        else if let Some(frame) = frames.last_mut() {
                            frame.bind(symbol, LocalBinding::Own(*term));
                        }
                        else {
                            session.defs.define_own(symbol, *term);
                        }
                    }
                    Some(Slot::Boolean(v)) => {
                        let term = session.builder().boolean(*v, Default::default());
                        if *delayed {
                            if let Some(frame) = frames.last_mut() {
                                frame.bind(symbol, LocalBinding::Own(term));
                            }
                            else {
                                session.defs.define_delayed(symbol, term);
                            }
                        }
                        else if let Some(frame) = frames.last_mut() {
                            frame.bind(symbol, LocalBinding::Own(term));
                        }
                        else {
                            session.defs.define_own(symbol, term);
                        }
                    }
                    _ => return Err(diag("write_value_unsupported")),
                }
                Ok(Slot::Unit)
            }
            OperationKind::WriteDownValue { key, pattern, value } => {
                let symbol = match slots.get(key) {
                    Some(Slot::Symbol(symbol)) => *symbol,
                    _ => return Err(diag("write_key_not_symbol")),
                };
                let pattern_term = match slots.get(pattern) {
                    Some(Slot::Term(term)) => *term,
                    _ => return Err(diag("write_pattern_not_term")),
                };
                let value_term = match slots.get(value) {
                    Some(Slot::Term(term)) => *term,
                    _ => return Err(diag("write_value_unsupported")),
                };
                // Bootstrap: DownValues attach to Session defs (not local ScopeFrame).
                session.defs.define_down_value(symbol, pattern_term, value_term);
                Ok(Slot::Unit)
            }
            OperationKind::ReadBinding { key } => {
                let symbol = match slots.get(key) {
                    Some(Slot::Symbol(symbol)) => *symbol,
                    _ => return Err(diag("read_key_not_symbol")),
                };
                for frame in frames.iter().rev() {
                    if let Some(LocalBinding::Own(term) | LocalBinding::Unique(term)) = frame.lookup(symbol) {
                        return Ok(Slot::Term(term));
                    }
                }
                if let Some(term) = session.defs.own(symbol) {
                    return Ok(Slot::Term(term));
                }
                if let Some(term) = session.defs.delayed(symbol) {
                    // Evaluate Delayed OwnValues on read (`DefineDeferred` semantics).
                    let module = ExecutionCompiler::new().compile(session, &AthenaRequest::Term(term))?;
                    let result_id = self.execute(session, &module, None)?;
                    let out = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(term);
                    return Ok(Slot::Term(out));
                }
                Ok(Slot::Term(session.builder().symbol_id(symbol, Default::default())))
            }
            OperationKind::CallProvider { call, .. } => {
                let descriptor = module.provider_calls.get(call.0 as usize).ok_or_else(|| diag("missing_provider_call"))?.clone();
                if descriptor.id != *call {
                    return Err(diag("provider_call_id_mismatch"));
                }
                let handoff = crate::execution::provider::ProviderCallHandoff::from_descriptor(descriptor);
                match provider.take() {
                    Some(domain) => {
                        let domain_result = execute_domain(session, domain)?;
                        let mut computation = computation_from_domain(session, domain_result);
                        computation =
                            computation.with_provenance(crate::runtime::results::ResultProvenance::call_provider(handoff.capabilities.fingerprint));
                        Ok(Slot::Result(session.insert_result(computation)))
                    }
                    None => {
                        let _ = handoff;
                        *unsupported = true;
                        Ok(Slot::Unit)
                    }
                }
            }
            OperationKind::PublishResult { source } => Ok(*slots.get(source).ok_or_else(|| diag("publish_source_undefined"))?),
            OperationKind::LoadInput { .. } | OperationKind::Guard { .. } | OperationKind::MaterializeValue { .. } => {
                Err(diag("operation_not_implemented"))
            }
        }
    }
}




#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::request::AthenaRequest,
        execution::{
            compiler::ExecutionCompiler,
            ir::{
                BasicBlock, BlockEdge, BlockId, ConstantId, ConstantValue, ExecutionValueType, ModuleFingerprint, Operation, Region, RegionId,
                Terminator,
            },
        },
    };

    #[test]
    fn execute_compiled_atom_term() {
        let mut session = Session::new();
        let term = session.builder().int(9, Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
                kind: OperationKind::Constant { constant: ConstantId(0) },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::Branch { condition: cond, then_edge: BlockEdge::jump(BlockId(1)), else_edge: BlockEdge::jump(BlockId(2)) },
        };
        let then_block = BasicBlock {
            id: BlockId(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(then_v),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(1) },
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
                kind: OperationKind::Constant { constant: ConstantId(2) },
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("branch");
        let loaded = session.results.get(result_id).expect("result");
        let term = loaded.symbolic_term.expect("term");
        match session.arena.get(term) {
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(true))) => {}
            other => panic!("expected true boolean term, got {other:?}"),
        }
    }

    #[test]
    fn truthy_and_or_with_zero_one() {
        let mut session = Session::new();
        let and = session.operators.intern("And");
        let or = session.operators.intern("Or");
        let z = session.builder().int(0, Default::default());
        let one = session.builder().int(1, Default::default());
        let and_term = session.builder().application(and, vec![z, one], Default::default());
        let or_term = session.builder().application(or, vec![z, one], Default::default());

        let and_mod = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(and_term)).expect("and");
        let and_id = ReferenceExecutor::new().execute(&mut session, &and_mod, None).expect("and exec");
        let and_out = session.results.get(and_id).expect("and result").symbolic_term.expect("term");
        match session.arena.get(and_out) {
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(false))) => {}
            other => panic!("expected And[0,1] == False, got {other:?}"),
        }

        let or_mod = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(or_term)).expect("or");
        let or_id = ReferenceExecutor::new().execute(&mut session, &or_mod, None).expect("or exec");
        let or_out = session.results.get(or_id).expect("or result").symbolic_term.expect("term");
        match session.arena.get(or_out) {
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(true))) => {}
            other => panic!("expected Or[0,1] == True, got {other:?}"),
        }
    }

    #[test]
    fn unknown_head_marks_partial_unknown() {
        let mut session = Session::new();
        let foo = session.operators.intern("FooBar");
        let one = session.builder().int(1, Default::default());
        let term = session.builder().application(foo, vec![one], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.status, ComputationStatus::Unknown);
        assert_eq!(loaded.coverage, CoverageStatus::Partial);
        assert!(loaded.diagnostics.is_empty());
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(athena_ir::TermNode::Application { head, .. }) if session.operators.name(*head) == Some("FooBar") => {}
            other => panic!("expected residual FooBar[...], got {other:?}"),
        }
    }

    #[test]
    fn part_oob_marks_invalid_index() {
        let mut session = Session::new();
        let part = session.operators.intern("Part");
        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(2, Default::default());
        let list = session.builder().list(vec![a, b], Default::default());
        let idx = session.builder().int(9, Default::default());
        let term = session.builder().application(part, vec![list, idx], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.status, ComputationStatus::Invalid);
        assert_eq!(loaded.diagnostics[0].code, DiagnosticCode::InvalidIndex);
    }

    #[test]
    fn part_span_extracts_slice() {
        let mut session = Session::new();
        let part = session.operators.intern("Part");
        let span = session.operators.intern("Span");
        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(2, Default::default());
        let c = session.builder().int(3, Default::default());
        let list = session.builder().list(vec![a, b, c], Default::default());
        let span_lo = session.builder().int(1, Default::default());
        let span_hi = session.builder().int(2, Default::default());
        let span_term = session.builder().application(span, vec![span_lo, span_hi], Default::default());
        let term = session.builder().application(part, vec![list, span_term], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
                assert_eq!(number_of(&session, items[0]).and_then(|n| n.as_exact_integer()), Some(1));
                assert_eq!(number_of(&session, items[1]).and_then(|n| n.as_exact_integer()), Some(2));
            }
            other => panic!("expected List[1, 2], got {other:?}"),
        }
    }

    #[test]
    fn part_column_all_then_index() {
        let mut session = Session::new();
        let part = session.operators.intern("Part");
        let all = session.builder().symbol("All", Default::default());
        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(2, Default::default());
        let c = session.builder().int(3, Default::default());
        let d = session.builder().int(4, Default::default());
        let row0 = session.builder().list(vec![a, b], Default::default());
        let row1 = session.builder().list(vec![c, d], Default::default());
        let matrix = session.builder().list(vec![row0, row1], Default::default());
        let col = session.builder().int(2, Default::default());
        let term = session.builder().application(part, vec![matrix, all, col], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
                assert_eq!(number_of(&session, items[0]).and_then(|n| n.as_exact_integer()), Some(2));
                assert_eq!(number_of(&session, items[1]).and_then(|n| n.as_exact_integer()), Some(4));
            }
            other => panic!("expected List[2, 4], got {other:?}"),
        }
    }
}
