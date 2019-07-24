//! `ReferenceExecutor` — correctness / replay backend for [`ExecutionModule`].
//!
//! Executes SSA blocks without an operand stack. Not a wrapper around the old VM.

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
        result = result.with_value(value).with_symbolic_term(term).with_provenance(ResultProvenance { request_kind: "ExecutionIR" });
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
                    "CollectMatches" | "Cases" => self.eval_collect_matches(session, args, slots),
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
                    // Evaluate Delayed OwnValues on read (SetDelayed semantics).
                    let module = ExecutionCompiler::new().compile(session, &AthenaRequest::Term(term))?;
                    let result_id = self.execute(session, &module, None)?;
                    let out = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(term);
                    return Ok(Slot::Term(out));
                }
                Ok(Slot::Term(session.builder().symbol_id(symbol, Default::default())))
            }
            OperationKind::CallProvider { call, .. } => {
                let _ = module.provider_calls.get(call.0 as usize).ok_or_else(|| diag("missing_provider_call"))?;
                match provider.take() {
                    Some(domain) => {
                        let domain_result = execute_domain(session, domain)?;
                        let computation = computation_from_domain(session, domain_result);
                        Ok(Slot::Result(session.insert_result(computation)))
                    }
                    None => {
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

    fn eval_unary_term_op(&self, session: &mut Session, name: &str, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 1 {
            return Err(diag("semantic_operator_arity"));
        }
        let slot = *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?;
        let term = self.slot_as_term(session, slot)?;
        match name {
            "Abs" => {
                if let Some(n) = number_of(session, term) {
                    Ok(Slot::Term(push_number(session, num_abs(clone_number(n)))))
                }
                else {
                    Ok(Slot::Term(push_application(session, "Abs", vec![term])))
                }
            }
            "Factorial" => {
                if let Some(n) = number_of(session, term) {
                    match num_factorial(n) {
                        Ok(v) => Ok(Slot::Term(push_number(session, v))),
                        Err(_) => Ok(Slot::Term(push_application(session, "Factorial", vec![term]))),
                    }
                }
                else {
                    Ok(Slot::Term(push_application(session, "Factorial", vec![term])))
                }
            }
            "Sqrt" => {
                if let Some(n) = number_of(session, term) {
                    match num_sqrt(n) {
                        Ok(Some(v)) => Ok(Slot::Term(push_number(session, v))),
                        _ => Ok(Slot::Term(push_application(session, "Sqrt", vec![term]))),
                    }
                }
                else {
                    Ok(Slot::Term(push_application(session, "Sqrt", vec![term])))
                }
            }
            "Length" => {
                let len = match session.arena.get(term) {
                    Some(athena_ir::TermNode::List(items)) => items.len() as i64,
                    Some(athena_ir::TermNode::Application { arguments, .. }) => arguments.len() as i64,
                    _ => return Ok(Slot::Term(push_application(session, "Length", vec![term]))),
                };
                Ok(Slot::Term(session.builder().int(len, Default::default())))
            }
            "First" => match session.arena.get(term) {
                Some(athena_ir::TermNode::List(items)) if !items.is_empty() => Ok(Slot::Term(items[0])),
                Some(athena_ir::TermNode::Application { arguments, .. }) if !arguments.is_empty() => Ok(Slot::Term(arguments[0])),
                Some(athena_ir::TermNode::List(_) | athena_ir::TermNode::Application { .. }) => Err(diag("first_empty")),
                _ => Ok(Slot::Term(push_application(session, "First", vec![term]))),
            },
            "Rest" => match session.arena.get(term) {
                Some(athena_ir::TermNode::List(items)) if !items.is_empty() => {
                    let rest = items[1..].to_vec();
                    Ok(Slot::Term(push_list(session, rest)))
                }
                Some(athena_ir::TermNode::Application { head, arguments }) if !arguments.is_empty() => {
                    let head = *head;
                    let rest = arguments[1..].to_vec();
                    Ok(Slot::Term(session.builder().application(head, rest, Default::default())))
                }
                Some(athena_ir::TermNode::List(_) | athena_ir::TermNode::Application { .. }) => Err(diag("rest_empty")),
                _ => Ok(Slot::Term(push_application(session, "Rest", vec![term]))),
            },
            _ => Err(diag("semantic_operator_not_implemented")),
        }
    }

    fn eval_residual_app(&self, session: &mut Session, name: &str, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        if terms.len() == 1 && matches!(name, "Sin" | "Cos" | "Tan" | "Exp" | "Log") {
            let arg = terms[0];
            if let Some(exact) = eval_trig_exact_session(session, name, arg) {
                return Ok(Slot::Term(exact));
            }
            if let Some(x) = term_as_f64_session(session, arg) {
                let y = match name {
                    "Sin" => x.sin(),
                    "Cos" => x.cos(),
                    "Tan" => x.tan(),
                    "Exp" => x.exp(),
                    "Log" => x.ln(),
                    _ => f64::NAN,
                };
                if y.is_finite() {
                    return Ok(Slot::Term(push_number(session, Number::machine(y))));
                }
            }
        }
        Ok(Slot::Term(push_application(session, name, terms)))
    }

    /// Apply the first matching Session DownValue rule and re-evaluate the rhs.
    fn try_apply_down_values(
        &self,
        session: &mut Session,
        name: &str,
        args: &[SsaValueId],
        slots: &HashMap<SsaValueId, Slot>,
    ) -> Result<Option<Slot>> {
        let symbol = session.arena.symbols_mut().intern(name);
        let Some(rules) = session.defs.down_values(symbol).map(<[(TermId, TermId)]>::to_vec)
        else {
            return Ok(None);
        };
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let substituted = {
            let mut matched = None;
            for (lhs, rhs) in rules {
                let Some(crate::execution::shape::Shape::Application(_, pat_args)) = crate::execution::shape::term_shape(session, lhs)
                else {
                    continue;
                };
                if pat_args.len() != terms.len() {
                    continue;
                }
                let mut binds = HashMap::new();
                if pat_args
                    .iter()
                    .zip(terms.iter())
                    .all(|(p, a)| crate::execution::builtins::patterns::pattern_bind(session, *a, *p, &mut binds))
                {
                    matched = Some(crate::execution::builtins::patterns::substitute_binds(session, rhs, &binds));
                    break;
                }
            }
            matched
        };
        let Some(substituted) = substituted
        else {
            return Ok(None);
        };
        let module = ExecutionCompiler::new().compile(session, &AthenaRequest::Term(substituted))?;
        let result_id = self.execute(session, &module, None)?;
        let out = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(substituted);
        Ok(Some(Slot::Term(out)))
    }

    fn eval_simplify(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 1 {
            return Err(diag("semantic_operator_arity"));
        }
        let term = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let evaluated = match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(term)) {
            Ok(module) => {
                let result_id = self.execute(session, &module, None)?;
                session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(term)
            }
            Err(_) => term,
        };
        if let Some(one) = try_pythagorean_session(session, evaluated) {
            return Ok(Slot::Term(one));
        }
        Ok(Slot::Term(evaluated))
    }

    /// Domain calculus heads (`D` / `Integrate` / …) via `try_calculus_request`.
    fn eval_calculus(&self, session: &mut Session, name: &str, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let echo = push_application(session, name, terms);
        let req = {
            let mut cc = CalculusCtx::new(session);
            try_calculus_request(&mut cc, echo)
        };
        if let Some(req) = req {
            let result = execute_calculus(session, req);
            let term = {
                let mut cc = CalculusCtx::new(session);
                materialize_calculus_result_term(&mut cc, &result)
            };
            return Ok(Slot::Term(term));
        }
        Ok(Slot::Term(echo))
    }

    fn eval_rule(&self, session: &mut Session, name: &str, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let lhs = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let rhs = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        Ok(Slot::Term(push_application(session, name, vec![lhs, rhs])))
    }

    fn eval_replace_all(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let expr = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let rules_term = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let rules = collect_rule_pairs(session, rules_term);
        if rules.is_empty() {
            return Ok(Slot::Term(push_application(session, "ReplaceAll", vec![expr, rules_term])));
        }
        let mut cur = expr;
        for (lhs, rhs) in rules {
            cur = crate::execution::builtins::patterns::replace_literal(session, cur, lhs, rhs);
        }
        match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(cur)) {
            Ok(module) => {
                let result_id = self.execute(session, &module, None)?;
                let term = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(cur);
                Ok(Slot::Term(term))
            }
            Err(_) => Ok(Slot::Term(cur)),
        }
    }

    /// `CollectMatches[list, pat]` / `Cases` — filter list items by pattern.
    fn eval_collect_matches(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let list = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let pat = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let Some(athena_ir::TermNode::List(items)) = session.arena.get(list)
        else {
            return Ok(Slot::Term(push_application(session, "CollectMatches", vec![list, pat])));
        };
        let items = items.clone();
        let mut out = Vec::new();
        for item in items {
            if crate::execution::builtins::patterns::pattern_matches(session, item, pat) {
                out.push(item);
            }
        }
        Ok(Slot::Term(push_list(session, out)))
    }

    /// `Matches[expr, pat]` — boolean pattern test.
    fn eval_matches(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let expr = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let pat = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let matched = crate::execution::builtins::patterns::pattern_matches(session, expr, pat);
        Ok(Slot::Boolean(matched))
    }

    fn eval_matrix_constructor(
        &self,
        session: &mut Session,
        name: &str,
        args: &[SsaValueId],
        slots: &HashMap<SsaValueId, Slot>,
    ) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let Some((rows, cols)) = parse_matrix_dims(session, &terms)
        else {
            return Ok(Slot::Term(push_application(session, name, terms)));
        };
        let n = match rows.checked_mul(cols) {
            Some(v) if v <= 4096 => v as usize,
            _ => return Ok(Slot::Term(push_application(session, name, terms))),
        };
        if n == 0 {
            return Ok(Slot::Term(push_list(session, Vec::new())));
        }
        let fill = match name {
            "Ones" => 1i64,
            "Zeros" | "Eye" => 0,
            _ => return Err(diag("semantic_operator_not_implemented")),
        };
        let mut rows_out = Vec::with_capacity(rows as usize);
        for r in 0..rows {
            let mut row = Vec::with_capacity(cols as usize);
            for c in 0..cols {
                let value = if name == "Eye" && r == c { 1 } else { fill };
                row.push(session.builder().int(value, Default::default()));
            }
            rows_out.push(push_list(session, row));
        }
        Ok(Slot::Term(push_list(session, rows_out)))
    }

    fn eval_map(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let func = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let list = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let items = match session.arena.get(list) {
            Some(athena_ir::TermNode::List(items)) => items.clone(),
            _ => return Ok(Slot::Term(push_application(session, "Map", vec![func, list]))),
        };
        // Bootstrap: symbol heads only (`Map[Abs, List[…]]`). Function/`Slot` later.
        let Some(name) = symbol_name(session, func)
        else {
            return Ok(Slot::Term(push_application(session, "Map", vec![func, list])));
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let mapped = push_application(session, &name, vec![item]);
            match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(mapped)) {
                Ok(module) => {
                    let result_id = self.execute(session, &module, None)?;
                    let term = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(mapped);
                    out.push(term);
                }
                Err(_) => out.push(mapped),
            }
        }
        Ok(Slot::Term(push_list(session, out)))
    }

    fn eval_apply(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let head = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let second = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let items = match session.arena.get(second) {
            Some(athena_ir::TermNode::List(items)) => items.clone(),
            _ => return Ok(Slot::Term(push_application(session, "Apply", vec![head, second]))),
        };
        let app = rebuild_application(session, head, items);
        match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(app)) {
            Ok(module) => {
                let result_id = self.execute(session, &module, None)?;
                let term = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(app);
                Ok(Slot::Term(term))
            }
            Err(_) => Ok(Slot::Term(app)),
        }
    }

    /// `Application[head, args…]` — apply `Function[var, body]` or symbol head.
    fn eval_application_form(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.is_empty() {
            return Err(diag("semantic_operator_arity"));
        }
        let head = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let mut call_args = Vec::with_capacity(args.len().saturating_sub(1));
        for id in &args[1..] {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            call_args.push(self.slot_as_term(session, slot)?);
        }
        // Function[var, body][arg…] → substitute and re-eval.
        if let Some(athena_ir::TermNode::Application { head: op, arguments }) = session.arena.get(head) {
            if session.operators.name(*op) == Some("Function") && arguments.len() == 2 && call_args.len() == 1 {
                let var = arguments[0];
                let body = arguments[1];
                if let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(sym))) = session.arena.get(var) {
                    let sym = *sym;
                    let instantiated = crate::execution::builtins::patterns::substitute_symbol(session, body, sym, call_args[0]);
                    match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(instantiated)) {
                        Ok(module) => {
                            let result_id = self.execute(session, &module, None)?;
                            let term = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(instantiated);
                            return Ok(Slot::Term(term));
                        }
                        Err(_) => return Ok(Slot::Term(instantiated)),
                    }
                }
            }
        }
        if let Some(name) = symbol_name(session, head) {
            let app = push_application(session, &name, call_args);
            match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(app)) {
                Ok(module) => {
                    let result_id = self.execute(session, &module, None)?;
                    let term = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(app);
                    return Ok(Slot::Term(term));
                }
                Err(_) => return Ok(Slot::Term(app)),
            }
        }
        let mut wrapped = Vec::with_capacity(call_args.len() + 1);
        wrapped.push(head);
        wrapped.extend(call_args);
        Ok(Slot::Term(push_application(session, "Application", wrapped)))
    }

    fn eval_size(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 1 {
            return Err(diag("semantic_operator_arity"));
        }
        let term = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let Some((rows, cols)) = nested_list_shape(session, term)
        else {
            return Ok(Slot::Term(push_application(session, "Size", vec![term])));
        };
        let r = session.builder().int(rows as i64, Default::default());
        let c = session.builder().int(cols as i64, Default::default());
        Ok(Slot::Term(push_list(session, vec![r, c])))
    }

    /// `Sum[list]` — vector scalar sum / matrix column sums (VM `array_sum` parity).
    /// `Sum[body, iterator]` — Table then Plus-fold.
    fn eval_sum(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() == 2 {
            let body = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
            let iter = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
            return match self.table_values(session, body, iter)? {
                Some(values) => {
                    if values.is_empty() {
                        Ok(Slot::Term(session.builder().int(0, Default::default())))
                    }
                    else {
                        Ok(Slot::Term(fold_plus_symbolic(session, values)))
                    }
                }
                None => Ok(Slot::Term(push_application(session, "Sum", vec![body, iter]))),
            };
        }
        if args.len() != 1 {
            return Ok(Slot::Term({
                let mut terms = Vec::with_capacity(args.len());
                for id in args {
                    let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
                    terms.push(self.slot_as_term(session, slot)?);
                }
                push_application(session, "Sum", terms)
            }));
        }
        let term = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let Some(athena_ir::TermNode::List(items)) = session.arena.get(term)
        else {
            return Ok(Slot::Term(push_application(session, "Sum", vec![term])));
        };
        let items = items.clone();
        if items.is_empty() {
            return Ok(Slot::Term(session.builder().int(0, Default::default())));
        }
        if matches!(session.arena.get(items[0]), Some(athena_ir::TermNode::List(_))) {
            // Matrix: sum each column into a row vector.
            let Some((_, cols)) = nested_list_shape(session, term)
            else {
                return Ok(Slot::Term(push_application(session, "Sum", vec![term])));
            };
            let mut out = Vec::with_capacity(cols as usize);
            for j in 0..cols as usize {
                let mut col = Vec::with_capacity(items.len());
                for row in &items {
                    let cell = match session.arena.get(*row) {
                        Some(athena_ir::TermNode::List(cells)) => cells.get(j).copied(),
                        _ => None,
                    };
                    let Some(cell) = cell
                    else {
                        return Ok(Slot::Term(push_application(session, "Sum", vec![term])));
                    };
                    col.push(cell);
                }
                out.push(fold_plus_symbolic(session, col));
            }
            return Ok(Slot::Term(push_list(session, out)));
        }
        // Vector: scalar sum via Plus fold.
        Ok(Slot::Term(fold_plus_symbolic(session, items)))
    }

    /// `Table[body, iterator]` — HoldAll-ish body with iterator expansion.
    fn eval_table(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let body = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let iter = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        match self.table_values(session, body, iter)? {
            Some(values) => Ok(Slot::Term(push_list(session, values))),
            None => Ok(Slot::Term(push_application(session, "Table", vec![body, iter]))),
        }
    }

    fn table_values(&self, session: &mut Session, body: TermId, iter: TermId) -> Result<Option<Vec<TermId>>> {
        let Some((var, values)) = expand_iterator_session(session, iter)
        else {
            return Ok(None);
        };
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            let instantiated = match var {
                Some(sym) => crate::execution::builtins::patterns::substitute_symbol(session, body, sym, value),
                None => body,
            };
            match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(instantiated)) {
                Ok(module) => {
                    let result_id = self.execute(session, &module, None)?;
                    let term = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(instantiated);
                    out.push(term);
                }
                Err(_) => out.push(instantiated),
            }
        }
        Ok(Some(out))
    }

    fn eval_det(
        &self,
        session: &mut Session,
        args: &[SsaValueId],
        slots: &HashMap<SsaValueId, Slot>,
        invalid: &mut Option<Diagnostic>,
    ) -> Result<Slot> {
        if args.len() != 1 {
            return Err(diag("semantic_operator_arity"));
        }
        let term = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let echo = push_application(session, "Det", vec![term]);
        let Some(matrix) = term_to_rational_matrix_session(session, term)
        else {
            return Ok(Slot::Term(echo));
        };
        match det_bareiss(&matrix) {
            Ok(result) => Ok(Slot::Term(rational_to_term_session(session, &result.det))),
            Err(diagnostic) => {
                *invalid = Some(diagnostic);
                Ok(Slot::Term(echo))
            }
        }
    }

    fn eval_linear_solve(
        &self,
        session: &mut Session,
        args: &[SsaValueId],
        slots: &HashMap<SsaValueId, Slot>,
        invalid: &mut Option<Diagnostic>,
    ) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let a = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let b = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let echo = push_application(session, "LinearSolve", vec![a, b]);
        let Some(am) = term_to_rational_matrix_session(session, a)
        else {
            return Ok(Slot::Term(echo));
        };
        let Some(bm) = term_to_rational_matrix_session(session, b)
        else {
            return Ok(Slot::Term(echo));
        };
        match solve_exact(&am, &bm) {
            Ok(sol) if sol.disposition == SolveDisposition::Unique => match sol.particular {
                Some(x) => match matrix_to_nested_list_session(session, &x) {
                    Ok(term) => Ok(Slot::Term(term)),
                    Err(diagnostic) => {
                        *invalid = Some(diagnostic);
                        Ok(Slot::Term(echo))
                    }
                },
                None => {
                    *invalid = Some(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", "LinearSolve"));
                    Ok(Slot::Term(echo))
                }
            },
            Ok(sol) => {
                let detail = match sol.disposition {
                    SolveDisposition::Inconsistent => "inconsistent",
                    SolveDisposition::Infinite { .. } => "underdetermined",
                    SolveDisposition::Unique => "unique",
                    SolveDisposition::Singular => "singular",
                    SolveDisposition::ResourceLimited => "resource_limited",
                };
                *invalid =
                    Some(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", "LinearSolve").detail("reason", detail));
                Ok(Slot::Term(echo))
            }
            Err(diagnostic) => {
                *invalid = Some(diagnostic);
                Ok(Slot::Term(echo))
            }
        }
    }

    fn eval_range(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let ints = terms.iter().map(|t| number_of(session, *t).and_then(|n| n.as_exact_integer())).collect::<Option<Vec<_>>>();
        let Some(ints) = ints
        else {
            return Ok(Slot::Term(push_application(session, "Range", terms)));
        };
        let bounds = match ints.as_slice() {
            [n] => Some((1, *n, 1)),
            [a, b] => Some((*a, *b, 1)),
            [a, b, step] => Some((*a, *b, *step)),
            _ => None,
        };
        let Some((a, b, step)) = bounds
        else {
            return Ok(Slot::Term(push_application(session, "Range", terms)));
        };
        let Some(values) = expand_span_3(a, step, b)
        else {
            return Ok(Slot::Term(push_application(session, "Range", terms)));
        };
        let out: Vec<TermId> = values.into_iter().map(|v| session.builder().int(v, Default::default())).collect();
        Ok(Slot::Term(push_list(session, out)))
    }

    fn eval_span(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let ints = terms.iter().map(|t| number_of(session, *t).and_then(|n| n.as_exact_integer())).collect::<Option<Vec<_>>>();
        let Some(ints) = ints
        else {
            return Ok(Slot::Term(push_application(session, "Span", terms)));
        };
        let items = match ints.as_slice() {
            [a, b] => expand_span_2(*a, *b),
            [a, step, b] => expand_span_3(*a, *step, *b),
            _ => return Ok(Slot::Term(push_application(session, "Span", terms))),
        };
        let Some(values) = items
        else {
            return Ok(Slot::Term(push_application(session, "Span", terms)));
        };
        let terms: Vec<TermId> = values.into_iter().map(|v| session.builder().int(v, Default::default())).collect();
        Ok(Slot::Term(push_list(session, terms)))
    }

    fn eval_part(
        &self,
        session: &mut Session,
        args: &[SsaValueId],
        slots: &HashMap<SsaValueId, Slot>,
        invalid: &mut Option<Diagnostic>,
    ) -> Result<Slot> {
        if args.len() < 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        // `Part[m, All, j, …]` — map remaining indices over each row (MATLAB `A(:,j)`).
        if terms.len() >= 3 {
            if let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol))) = session.arena.get(terms[1]) {
                if matches!(session.arena.symbols().resolve(*symbol), Some("All") | Some(":")) {
                    if let Some(athena_ir::TermNode::List(rows)) = session.arena.get(terms[0]) {
                        let rows = rows.clone();
                        let rest = terms[2..].to_vec();
                        let mut out = Vec::with_capacity(rows.len());
                        for row in rows {
                            let mut part_args = Vec::with_capacity(1 + rest.len());
                            part_args.push(row);
                            part_args.extend_from_slice(&rest);
                            match self.part_n_terms(session, &part_args, invalid)? {
                                PartStep::Next(item) => out.push(item),
                                PartStep::Residual => {
                                    return Ok(Slot::Term(push_application(session, "Part", terms)));
                                }
                                PartStep::Invalid { echo, diagnostic } => {
                                    *invalid = Some(diagnostic);
                                    return Ok(Slot::Term(echo));
                                }
                            }
                        }
                        return Ok(Slot::Term(push_list(session, out)));
                    }
                }
            }
        }
        match self.part_n_terms(session, &terms, invalid)? {
            PartStep::Next(term) => Ok(Slot::Term(term)),
            PartStep::Residual => Ok(Slot::Term(push_application(session, "Part", terms))),
            PartStep::Invalid { echo, diagnostic } => {
                *invalid = Some(diagnostic);
                Ok(Slot::Term(echo))
            }
        }
    }

    fn part_n_terms(&self, session: &mut Session, terms: &[TermId], invalid: &mut Option<Diagnostic>) -> Result<PartStep> {
        if terms.len() < 2 {
            return Ok(PartStep::Residual);
        }
        let mut cur = terms[0];
        for index in &terms[1..] {
            match self.part_one(session, cur, *index)? {
                PartStep::Next(next) => cur = next,
                PartStep::Residual => return Ok(PartStep::Residual),
                PartStep::Invalid { echo, diagnostic } => {
                    *invalid = Some(diagnostic.clone());
                    return Ok(PartStep::Invalid { echo, diagnostic });
                }
            }
        }
        Ok(PartStep::Next(cur))
    }

    /// Bootstrap `Part` step: list/app + integer / `End` / `All` / index list.
    fn part_one(&self, session: &mut Session, expr: TermId, index: TermId) -> Result<PartStep> {
        // Index list (e.g. evaluated `Span`): extract each position into a list.
        if let Some(athena_ir::TermNode::List(indices)) = session.arena.get(index) {
            let indices = indices.clone();
            let mut out = Vec::with_capacity(indices.len());
            for idx in indices {
                match self.part_one(session, expr, idx)? {
                    PartStep::Next(item) => out.push(item),
                    PartStep::Residual => {
                        return Ok(PartStep::Residual);
                    }
                    PartStep::Invalid { echo, diagnostic } => {
                        return Ok(PartStep::Invalid { echo, diagnostic });
                    }
                }
            }
            return Ok(PartStep::Next(push_list(session, out)));
        }

        let items = match session.arena.get(expr) {
            Some(athena_ir::TermNode::List(items)) => items.clone(),
            Some(athena_ir::TermNode::Application { arguments, .. }) => arguments.clone(),
            _ => return Ok(PartStep::Residual),
        };
        if let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol))) = session.arena.get(index) {
            match session.arena.symbols().resolve(*symbol) {
                Some("End") | Some("end") => {
                    return Ok(match items.last().copied() {
                        Some(item) => PartStep::Next(item),
                        None => PartStep::Residual,
                    });
                }
                Some("All") | Some(":") => {
                    return Ok(PartStep::Next(push_list(session, items)));
                }
                _ => {}
            }
        }
        let Some(idx) = number_of(session, index).and_then(|n| n.as_exact_integer())
        else {
            return Ok(PartStep::Residual);
        };
        if idx == 0 {
            return Ok(PartStep::Next(match session.arena.get(expr) {
                Some(athena_ir::TermNode::List(_)) => session.builder().symbol("List", Default::default()),
                Some(athena_ir::TermNode::Application { head, .. }) => {
                    let name = session.operators.name(*head).unwrap_or("").to_string();
                    session.builder().symbol(&name, Default::default())
                }
                _ => return Ok(PartStep::Residual),
            }));
        }
        let len = items.len();
        let pos = if idx > 0 {
            (idx - 1) as usize
        }
        else {
            let pos = len as i64 + idx;
            if pos < 0 {
                let echo = push_application(session, "Part", vec![expr, index]);
                return Ok(PartStep::Invalid { echo, diagnostic: crate::diagnostics::invalid_index_diagnostic(idx, Some(len as u64)) });
            }
            pos as usize
        };
        match items.get(pos) {
            Some(item) => Ok(PartStep::Next(*item)),
            None => {
                let echo = push_application(session, "Part", vec![expr, index]);
                Ok(PartStep::Invalid { echo, diagnostic: crate::diagnostics::invalid_index_diagnostic(idx, Some(len as u64)) })
            }
        }
    }

    fn eval_join(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        let mut out = Vec::new();
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            let term = self.slot_as_term(session, slot)?;
            terms.push(term);
            match session.arena.get(term) {
                Some(athena_ir::TermNode::List(items)) => out.extend_from_slice(items),
                _ => return Ok(Slot::Term(push_application(session, "Join", terms))),
            }
        }
        Ok(Slot::Term(push_list(session, out)))
    }

    fn eval_compare_chain(&self, session: &mut Session, name: &str, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() < 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let pick = match name {
            "Less" => |o: Ordering| o == Ordering::Less,
            "Greater" => |o: Ordering| o == Ordering::Greater,
            "LessEqual" => |o: Ordering| o != Ordering::Greater,
            "GreaterEqual" => |o: Ordering| o != Ordering::Less,
            _ => return Err(diag("semantic_operator_not_implemented")),
        };
        // Binary list broadcast (VM `eval_compare` parity).
        if terms.len() == 2 {
            if let Some(broadcast) = compare_list_broadcast(session, name, terms[0], terms[1], pick)? {
                return Ok(Slot::Term(broadcast));
            }
        }
        let numbers = terms.iter().map(|t| number_of(session, *t).map(clone_number)).collect::<Option<Vec<_>>>();
        let Some(nums) = numbers
        else {
            return Ok(Slot::Term(push_application(session, name, terms)));
        };
        let mut ok = true;
        for window in nums.windows(2) {
            let ord = num_compare(&window[0], &window[1]).ok_or_else(|| diag("compare_failed"))?;
            if !pick(ord) {
                ok = false;
                break;
            }
        }
        Ok(Slot::Boolean(ok))
    }

    fn eval_arithmetic(&self, session: &mut Session, name: &str, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let numbers = terms.iter().map(|t| number_of(session, *t).map(clone_number)).collect::<Option<Vec<_>>>();
        if let Some(nums) = numbers {
            let folded = match (name, nums.as_slice()) {
                ("Plus", []) => Some(Number::small_int(0)),
                ("Plus", values) => {
                    let mut acc = clone_number(&values[0]);
                    let mut ok = true;
                    for n in &values[1..] {
                        match num_add(clone_number(&acc), clone_number(n)) {
                            Ok(v) => acc = v,
                            Err(_) => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    ok.then_some(acc)
                }
                ("Times", []) => Some(Number::small_int(1)),
                ("Times", values) => {
                    let mut acc = clone_number(&values[0]);
                    let mut ok = true;
                    for n in &values[1..] {
                        match num_mul(clone_number(&acc), clone_number(n)) {
                            Ok(v) => acc = v,
                            Err(_) => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    ok.then_some(acc)
                }
                ("Subtract", [a]) => num_mul(Number::small_int(-1), clone_number(a)).ok(),
                ("Subtract", [a, b]) => num_mul(Number::small_int(-1), clone_number(b)).and_then(|neg| num_add(clone_number(a), neg)).ok(),
                ("Divide", [a, b]) => num_div(clone_number(a), clone_number(b)).ok(),
                ("Power", [a, b]) => num_pow(a, b).ok(),
                _ => return Err(diag("semantic_operator_arity")),
            };
            if let Some(folded) = folded {
                return Ok(Slot::Term(push_number(session, folded)));
            }
            // Numeric fold failed (e.g. `0^-1`) — keep symbolic residual.
        }
        // Symbolic residual with identity folding for Plus/Times/Power/Divide.
        Ok(Slot::Term(match name {
            "Plus" => fold_plus_symbolic(session, terms),
            "Times" => fold_times_symbolic(session, terms),
            "Power" => fold_power_symbolic(session, terms),
            "Divide" => fold_divide_symbolic(session, terms),
            "Subtract" => fold_subtract_symbolic(session, terms),
            _ => push_application(session, name, terms),
        }))
    }

    fn slot_as_term(&self, session: &mut Session, slot: Slot) -> Result<TermId> {
        match slot {
            Slot::Term(term) => Ok(term),
            Slot::Boolean(value) => Ok(session.builder().boolean(value, Default::default())),
            Slot::Symbol(symbol) => Ok(session.builder().symbol_id(symbol, Default::default())),
            Slot::Unit => Ok(session.builder().null(Default::default())),
            Slot::Scope(_) | Slot::Result(_) => Err(diag("slot_not_term")),
        }
    }
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("component", "ReferenceExecutor").detail("reason", reason)
}

/// Binary list broadcast for compares. Returns `None` when neither side is a list.
fn compare_list_broadcast(
    session: &mut Session,
    name: &str,
    left: TermId,
    right: TermId,
    pick: fn(Ordering) -> bool,
) -> Result<Option<TermId>> {
    let l_list = matches!(session.arena.get(left), Some(athena_ir::TermNode::List(_)));
    let r_list = matches!(session.arena.get(right), Some(athena_ir::TermNode::List(_)));
    match (l_list, r_list) {
        (false, false) => Ok(None),
        (true, true) => {
            let xs = match session.arena.get(left) {
                Some(athena_ir::TermNode::List(items)) => items.clone(),
                _ => return Ok(None),
            };
            let ys = match session.arena.get(right) {
                Some(athena_ir::TermNode::List(items)) => items.clone(),
                _ => return Ok(None),
            };
            if xs.len() != ys.len() {
                return Ok(Some(push_application(session, name, vec![left, right])));
            }
            let mut out = Vec::with_capacity(xs.len());
            for (a, b) in xs.into_iter().zip(ys.into_iter()) {
                out.push(compare_pair_term(session, name, a, b, pick)?);
            }
            Ok(Some(push_list(session, out)))
        }
        (true, false) => {
            let xs = match session.arena.get(left) {
                Some(athena_ir::TermNode::List(items)) => items.clone(),
                _ => return Ok(None),
            };
            let mut out = Vec::with_capacity(xs.len());
            for a in xs {
                out.push(compare_pair_term(session, name, a, right, pick)?);
            }
            Ok(Some(push_list(session, out)))
        }
        (false, true) => {
            let ys = match session.arena.get(right) {
                Some(athena_ir::TermNode::List(items)) => items.clone(),
                _ => return Ok(None),
            };
            let mut out = Vec::with_capacity(ys.len());
            for b in ys {
                out.push(compare_pair_term(session, name, left, b, pick)?);
            }
            Ok(Some(push_list(session, out)))
        }
    }
}

fn compare_pair_term(session: &mut Session, name: &str, left: TermId, right: TermId, pick: fn(Ordering) -> bool) -> Result<TermId> {
    // Nested lists recurse through broadcast.
    if matches!(session.arena.get(left), Some(athena_ir::TermNode::List(_)))
        || matches!(session.arena.get(right), Some(athena_ir::TermNode::List(_)))
    {
        return Ok(
            compare_list_broadcast(session, name, left, right, pick)?.unwrap_or_else(|| push_application(session, name, vec![left, right]))
        );
    }
    match (number_of(session, left).map(clone_number), number_of(session, right).map(clone_number)) {
        (Some(a), Some(b)) => {
            let ord = num_compare(&a, &b).ok_or_else(|| diag("compare_failed"))?;
            Ok(session.builder().boolean(pick(ord), Default::default()))
        }
        _ => Ok(push_application(session, name, vec![left, right])),
    }
}

fn is_known_residual_head(name: &str) -> bool {
    matches!(
        name,
        "Sin"
            | "Cos"
            | "Tan"
            | "Exp"
            | "Log"
            | "Sinh"
            | "Cosh"
            | "Tanh"
            | "ArcSin"
            | "ArcCos"
            | "ArcTan"
            | "Erf"
            | "Gamma"
            | "D"
            | "Integrate"
            | "Hold"
            | "HoldForm"
            | "Function"
    )
}

/// Logic ops: Boolean atoms · `True`/`False` · exact `0`/`1` (VM `as_boolean_id` parity).
fn slot_as_boolean_like(session: &Session, slot: Slot) -> Option<bool> {
    match slot {
        Slot::Boolean(v) => Some(v),
        Slot::Term(term) => as_boolean_like_term(session, term),
        _ => None,
    }
}

fn as_boolean_like_term(session: &Session, term: TermId) -> Option<bool> {
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(v))) => Some(*v),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol))) => match session.arena.symbols().resolve(*symbol) {
            Some("True") => Some(true),
            Some("False") => Some(false),
            _ => None,
        },
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) => {
            if n.is_zero() {
                Some(false)
            }
            else if *n == Number::small_int(1) {
                Some(true)
            }
            else {
                None
            }
        }
        _ => None,
    }
}

fn coerce_branch_predicate(session: &Session, term: TermId) -> Result<bool> {
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(v))) => Ok(*v),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol))) => match session.arena.symbols().resolve(*symbol) {
            Some("True") => Ok(true),
            Some("False") => Ok(false),
            _ => Err(Diagnostic::new(DiagnosticCode::NonBooleanCondition)
                .detail("component", "ReferenceExecutor")
                .detail("reason", "branch_symbol_not_boolean")),
        },
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) => Ok(!n.is_zero()),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Null)) => Ok(false),
        _ => Err(Diagnostic::new(DiagnosticCode::NonBooleanCondition)
            .detail("component", "ReferenceExecutor")
            .detail("reason", "branch_term_not_boolean")),
    }
}

fn fold_plus_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
    // Flatten one level of nested `Plus` and coalesce numeric summands.
    let mut flat = Vec::with_capacity(terms.len());
    let mut sum: Option<Number> = None;
    for term in terms {
        match session.arena.get(term) {
            Some(athena_ir::TermNode::Application { head, arguments }) if session.operators.name(*head) == Some("Plus") => {
                for arg in arguments.clone() {
                    push_plus_summand_session(session, arg, &mut flat, &mut sum);
                }
            }
            _ => push_plus_summand_session(session, term, &mut flat, &mut sum),
        }
    }
    if let Some(s) = sum {
        if !s.is_zero() {
            flat.insert(0, push_number(session, s));
        }
    }
    let flat = combine_like_plus_session(session, flat);
    match flat.as_slice() {
        [] => session.builder().int(0, Default::default()),
        [only] => *only,
        _ => push_application(session, "Plus", flat),
    }
}

fn push_plus_summand_session(session: &mut Session, term: TermId, flat: &mut Vec<TermId>, sum: &mut Option<Number>) {
    if let Some(n) = number_of(session, term) {
        let n = clone_number(n);
        *sum = Some(match sum.take() {
            Some(s) => num_add(clone_number(&s), n).unwrap_or(s),
            None => n,
        });
    }
    else {
        flat.push(term);
    }
}

/// Merge `c1·k + c2·k` (bare `k` as coefficient 1).
fn combine_like_plus_session(session: &mut Session, terms: Vec<TermId>) -> Vec<TermId> {
    let mut groups: Vec<(TermId, Number)> = Vec::new();
    for t in terms {
        let (coef, kernel) = split_numeric_coeff_session(session, t);
        let mut matched = false;
        for (k, acc) in groups.iter_mut() {
            if session.arena.structural_eq(*k, kernel) {
                match num_add(clone_number(acc), clone_number(&coef)) {
                    Ok(v) => *acc = v,
                    Err(_) => return groups_to_plus_terms_session(session, groups),
                }
                matched = true;
                break;
            }
        }
        if !matched {
            groups.push((kernel, coef));
        }
    }
    groups_to_plus_terms_session(session, groups)
}

fn split_numeric_coeff_session(session: &mut Session, term: TermId) -> (Number, TermId) {
    if let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(term) {
        if session.operators.name(*head) == Some("Times") && !arguments.is_empty() {
            let args = arguments.clone();
            let mut coef = Number::small_int(1);
            let mut rest = Vec::new();
            for a in args {
                if let Some(n) = number_of(session, a) {
                    coef = num_mul(clone_number(&coef), clone_number(n)).unwrap_or(coef);
                }
                else {
                    rest.push(a);
                }
            }
            let kernel = match rest.as_slice() {
                [] => session.builder().int(1, Default::default()),
                [only] => *only,
                _ => push_application(session, "Times", rest),
            };
            return (coef, kernel);
        }
    }
    if let Some(n) = number_of(session, term) {
        return (clone_number(n), session.builder().int(1, Default::default()));
    }
    (Number::small_int(1), term)
}

fn groups_to_plus_terms_session(session: &mut Session, groups: Vec<(TermId, Number)>) -> Vec<TermId> {
    let mut out = Vec::new();
    for (kernel, coef) in groups {
        if coef.is_zero() {
            continue;
        }
        else if number_of(session, kernel).is_some_and(Number::is_one) {
            out.push(push_number(session, coef));
        }
        else if coef.is_one() {
            out.push(kernel);
        }
        else {
            let coef_id = push_number(session, coef);
            out.push(fold_times_symbolic(session, vec![coef_id, kernel]));
        }
    }
    out
}

fn fold_times_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
    // Flatten one level of nested `Times`.
    let mut flat = Vec::with_capacity(terms.len());
    for term in terms {
        match session.arena.get(term) {
            Some(athena_ir::TermNode::Application { head, arguments }) if session.operators.name(*head) == Some("Times") => {
                flat.extend_from_slice(arguments);
            }
            _ => flat.push(term),
        }
    }
    if flat.iter().any(|t| number_of(session, *t).is_some_and(Number::is_zero)) {
        return session.builder().int(0, Default::default());
    }
    let mut out = Vec::with_capacity(flat.len());
    for term in flat {
        if number_of(session, term).is_some_and(Number::is_one) {
            continue;
        }
        out.push(term);
    }
    let out = combine_like_powers_session(session, out);
    let out = canonicalize_times_factors_session(session, out);
    // One-level distribute: `c * (a + b) → c*a + c*b`.
    if let Some(idx) = out.iter().position(|t| {
        matches!(
            session.arena.get(*t),
            Some(athena_ir::TermNode::Application { head, .. })
                if session.operators.name(*head) == Some("Plus")
        )
    }) {
        let plus_id = out[idx];
        let mut factors = out.clone();
        factors.remove(idx);
        if let Some(athena_ir::TermNode::Application { arguments, .. }) = session.arena.get(plus_id) {
            let summands = arguments.clone();
            let parts: Vec<TermId> = summands
                .into_iter()
                .map(|s| {
                    let mut f = factors.clone();
                    f.push(s);
                    fold_times_symbolic(session, f)
                })
                .collect();
            return fold_plus_symbolic(session, parts);
        }
    }
    match out.as_slice() {
        [] => session.builder().int(1, Default::default()),
        [only] => *only,
        _ => push_application(session, "Times", out),
    }
}

fn canonicalize_times_factors_session(session: &mut Session, factors: Vec<TermId>) -> Vec<TermId> {
    let mut product: Option<Number> = None;
    let mut rest = Vec::new();
    for f in factors {
        if let Some(n) = number_of(session, f) {
            let n = clone_number(n);
            product = Some(match product.take() {
                Some(p) => num_mul(clone_number(&p), n).unwrap_or(p),
                None => n,
            });
        }
        else {
            rest.push(f);
        }
    }
    let mut out = Vec::new();
    if let Some(p) = product {
        if !p.is_one() {
            out.push(push_number(session, p));
        }
    }
    out.extend(rest);
    out
}

/// Merge `Power[b,e1] * Power[b,e2]` (bare symbol as `Power[b,1]`).
fn combine_like_powers_session(session: &mut Session, factors: Vec<TermId>) -> Vec<TermId> {
    let mut groups: Vec<(TermId, TermId)> = Vec::new();
    let mut rest = Vec::new();
    for f in factors {
        let base_exp = match session.arena.get(f) {
            Some(athena_ir::TermNode::Application { head, arguments })
                if session.operators.name(*head) == Some("Power") && arguments.len() == 2 =>
            {
                Some((arguments[0], arguments[1]))
            }
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(_))) => Some((f, session.builder().int(1, Default::default()))),
            _ => None,
        };
        match base_exp {
            Some((base, exp)) => {
                let mut merged = false;
                for (b, e) in groups.iter_mut() {
                    if session.arena.structural_eq(*b, base) {
                        let combined = match (number_of(session, *e), number_of(session, exp)) {
                            (Some(a), Some(b)) => match num_add(clone_number(a), clone_number(b)) {
                                Ok(v) => push_number(session, v),
                                Err(_) => push_application(session, "Plus", vec![*e, exp]),
                            },
                            _ => push_application(session, "Plus", vec![*e, exp]),
                        };
                        *e = combined;
                        merged = true;
                        break;
                    }
                }
                if !merged {
                    groups.push((base, exp));
                }
            }
            None => rest.push(f),
        }
    }
    let mut merged = Vec::new();
    for (base, exp) in groups {
        let p = fold_power_symbolic(session, vec![base, exp]);
        if number_of(session, p).is_some_and(Number::is_one) {
            continue;
        }
        merged.push(p);
    }
    merged.extend(rest);
    merged
}

fn fold_divide_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
    if terms.len() != 2 {
        return push_application(session, "Divide", terms);
    }
    let (num, den) = (terms[0], terms[1]);
    let neg1 = session.builder().int(-1, Default::default());
    let inv = fold_power_symbolic(session, vec![den, neg1]);
    fold_times_symbolic(session, vec![num, inv])
}

fn fold_subtract_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
    match terms.as_slice() {
        [a] => {
            let neg1 = session.builder().int(-1, Default::default());
            fold_times_symbolic(session, vec![neg1, *a])
        }
        [a, b] => {
            let neg1 = session.builder().int(-1, Default::default());
            let neg = fold_times_symbolic(session, vec![neg1, *b]);
            fold_plus_symbolic(session, vec![*a, neg])
        }
        _ => push_application(session, "Subtract", terms),
    }
}

fn fold_power_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
    if terms.len() != 2 {
        return push_application(session, "Power", terms);
    }
    let (base, exp) = (terms[0], terms[1]);
    if let Some(e) = number_of(session, exp) {
        if e.is_zero() {
            // Scalar `x^0 → 1`; list bases stay residual (elementwise is `DotPower`).
            if matches!(session.arena.get(base), Some(athena_ir::TermNode::List(_))) {
                return push_application(session, "Power", terms);
            }
            return session.builder().int(1, Default::default());
        }
        if e.is_one() {
            return base;
        }
        // `(u^a)^b → u^(a*b)` and `(c*u)^n → c^n * u^n` when exponents are integers.
        if e.as_integer_exp().is_some() {
            if let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(base) {
                let head_name = session.operators.name(*head);
                if head_name == Some("Power") && arguments.len() == 2 {
                    let inner_base = arguments[0];
                    if let Some(inner_exp) = number_of(session, arguments[1]) {
                        if let Ok(combined) = num_mul(clone_number(inner_exp), clone_number(e)) {
                            let combined_id = push_number(session, combined);
                            return fold_power_symbolic(session, vec![inner_base, combined_id]);
                        }
                    }
                }
                if head_name == Some("Times") && arguments.len() >= 2 {
                    let args = arguments.clone();
                    if let Some(c) = number_of(session, args[0]) {
                        if let Ok(cp) = num_pow(c, e) {
                            let rest = if args.len() == 2 { args[1] } else { push_application(session, "Times", args[1..].to_vec()) };
                            let rest_pow = fold_power_symbolic(session, vec![rest, exp]);
                            let cp_id = push_number(session, cp);
                            return fold_times_symbolic(session, vec![cp_id, rest_pow]);
                        }
                    }
                }
            }
        }
    }
    push_application(session, "Power", terms)
}

fn eval_trig_exact_session(session: &mut Session, name: &str, arg: TermId) -> Option<TermId> {
    let angle = normalize_pi_angle_session(session, arg)?;
    match name {
        "Sin" => Some(session.builder().int(0, Default::default())),
        "Cos" => Some(session.builder().int(if angle % 2 == 0 { 1 } else { -1 }, Default::default())),
        "Tan" if angle % 2 == 0 => Some(session.builder().int(0, Default::default())),
        _ => None,
    }
}

fn term_as_f64_session(session: &Session, arg: TermId) -> Option<f64> {
    if let Some(k) = normalize_pi_angle_session(session, arg) {
        return Some((k as f64) * std::f64::consts::PI);
    }
    if head_name_session(session, arg).as_deref() == Some("E") {
        return Some(std::f64::consts::E);
    }
    number_of(session, arg).and_then(num_to_f64_lossy)
}

fn normalize_pi_angle_session(session: &Session, arg: TermId) -> Option<i64> {
    if let Some(n) = number_of(session, arg).and_then(|n| n.as_exact_integer()) {
        if n == 0 {
            return Some(0);
        }
    }
    if head_name_session(session, arg).as_deref() == Some("Pi") {
        return Some(1);
    }
    if let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(arg) {
        if session.operators.name(*head) == Some("Times") {
            if let [a, b] = arguments.as_slice() {
                if head_name_session(session, *a).as_deref() == Some("Pi") {
                    return number_of(session, *b).and_then(|n| n.as_exact_integer());
                }
                if head_name_session(session, *b).as_deref() == Some("Pi") {
                    return number_of(session, *a).and_then(|n| n.as_exact_integer());
                }
            }
        }
        if session.operators.name(*head) == Some("Plus")
            && arguments.len() == 1
            && head_name_session(session, arguments[0]).as_deref() == Some("Pi")
        {
            return Some(1);
        }
    }
    None
}

fn head_name_session(session: &Session, id: TermId) -> Option<String> {
    match session.arena.get(id)? {
        athena_ir::TermNode::Application { head, .. } => session.operators.name(*head).map(str::to_string),
        athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol)) => session.arena.symbols().resolve(*symbol).map(str::to_string),
        _ => None,
    }
}

fn expand_span_2(a: i64, b: i64) -> Option<Vec<i64>> {
    let mut out = Vec::new();
    if a <= b {
        let mut x = a;
        while x <= b {
            out.push(x);
            x += 1;
        }
    }
    else {
        let mut x = a;
        while x >= b {
            out.push(x);
            x -= 1;
        }
    }
    Some(out)
}

fn expand_span_3(a: i64, step: i64, b: i64) -> Option<Vec<i64>> {
    if step == 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut x = a;
    if step > 0 {
        while x <= b {
            out.push(x);
            x += step;
        }
    }
    else {
        while x >= b {
            out.push(x);
            x += step;
        }
    }
    Some(out)
}

/// Expand `{i,n}` / `{i,a,b}` / `{i,a,b,step}` / `{n}` for `Table` / iterator `Sum`.
fn expand_iterator_session(session: &mut Session, spec: TermId) -> Option<(Option<SymbolId>, Vec<TermId>)> {
    let items = match session.arena.get(spec) {
        Some(athena_ir::TermNode::List(items)) => items.clone(),
        _ => return None,
    };
    match items.as_slice() {
        [var, n] => {
            let sym = term_symbol_id(session, *var)?;
            let n = number_of(session, *n)?.as_exact_integer()?;
            Some((Some(sym), range_int_terms(session, 1, n, 1)?))
        }
        [var, a, b] => {
            let sym = term_symbol_id(session, *var)?;
            let a = number_of(session, *a)?.as_exact_integer()?;
            let b = number_of(session, *b)?.as_exact_integer()?;
            Some((Some(sym), range_int_terms(session, a, b, 1)?))
        }
        [var, a, b, step] => {
            let sym = term_symbol_id(session, *var)?;
            let a = number_of(session, *a)?.as_exact_integer()?;
            let b = number_of(session, *b)?.as_exact_integer()?;
            let step = number_of(session, *step)?.as_exact_integer()?;
            Some((Some(sym), range_int_terms(session, a, b, step)?))
        }
        [n] => {
            let n = number_of(session, *n)?.as_exact_integer()?;
            Some((None, range_int_terms(session, 1, n, 1)?))
        }
        _ => None,
    }
}

fn term_symbol_id(session: &Session, id: TermId) -> Option<SymbolId> {
    match session.arena.get(id) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(s))) => Some(*s),
        _ => None,
    }
}

fn range_int_terms(session: &mut Session, a: i64, b: i64, step: i64) -> Option<Vec<TermId>> {
    let ints = expand_span_3(a, step, b)?;
    Some(ints.into_iter().map(|n| session.builder().int(n, Default::default())).collect())
}

fn rebuild_application(session: &mut Session, head: TermId, args: Vec<TermId>) -> TermId {
    match session.arena.get(head) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol))) => {
            let name = session.arena.symbols().resolve(*symbol).unwrap_or("?").to_string();
            push_application(session, &name, args)
        }
        _ => {
            let mut wrapped = Vec::with_capacity(args.len() + 1);
            wrapped.push(head);
            wrapped.extend(args);
            push_application(session, "Application", wrapped)
        }
    }
}

fn symbol_name(session: &Session, id: TermId) -> Option<String> {
    match session.arena.get(id) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol))) => session.arena.symbols().resolve(*symbol).map(str::to_string),
        _ => None,
    }
}

fn parse_matrix_dims(session: &Session, args: &[TermId]) -> Option<(u64, u64)> {
    let as_dim = |t: TermId| -> Option<u64> {
        let n = number_of(session, t)?.as_exact_integer()?;
        if n < 0 { None } else { Some(n as u64) }
    };
    match args {
        [n] => {
            let n = as_dim(*n)?;
            Some((n, n))
        }
        [m, n] => Some((as_dim(*m)?, as_dim(*n)?)),
        _ => None,
    }
}

fn collect_rule_pairs(session: &Session, rules_term: TermId) -> Vec<(TermId, TermId)> {
    match session.arena.get(rules_term) {
        Some(athena_ir::TermNode::List(items)) => items.iter().filter_map(|r| rule_pair(session, *r)).collect(),
        _ => rule_pair(session, rules_term).into_iter().collect(),
    }
}

fn rule_pair(session: &Session, expr: TermId) -> Option<(TermId, TermId)> {
    let athena_ir::TermNode::Application { head, arguments } = session.arena.get(expr)?
    else {
        return None;
    };
    if arguments.len() != 2 {
        return None;
    }
    let name = session.operators.name(*head)?;
    if matches!(name, "Rule" | "RuleDeferred") { Some((arguments[0], arguments[1])) } else { None }
}

fn try_pythagorean_session(session: &mut Session, expr: TermId) -> Option<TermId> {
    let athena_ir::TermNode::Application { head, arguments } = session.arena.get(expr)?
    else {
        return None;
    };
    if session.operators.name(*head) != Some("Plus") || arguments.len() != 2 {
        return None;
    }
    let (a, b) = (arguments[0], arguments[1]);
    if is_trig_sq_session(session, a, "Sin") && is_trig_sq_session(session, b, "Cos") && same_trig_arg_session(session, a, b) {
        return Some(session.builder().int(1, Default::default()));
    }
    if is_trig_sq_session(session, a, "Cos") && is_trig_sq_session(session, b, "Sin") && same_trig_arg_session(session, a, b) {
        return Some(session.builder().int(1, Default::default()));
    }
    None
}

fn is_trig_sq_session(session: &Session, expr: TermId, name: &str) -> bool {
    let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(expr)
    else {
        return false;
    };
    if arguments.len() != 2 || session.operators.name(*head) != Some("Power") {
        return false;
    }
    let exp_is_two = matches!(
        session.arena.get(arguments[1]),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) if n.as_exact_integer() == Some(2)
    );
    if !exp_is_two {
        return false;
    }
    match session.arena.get(arguments[0]) {
        Some(athena_ir::TermNode::Application { head, arguments: inner }) if inner.len() == 1 => session.operators.name(*head) == Some(name),
        _ => false,
    }
}

fn same_trig_arg_session(session: &Session, a: TermId, b: TermId) -> bool {
    let arg = |expr: TermId| -> Option<TermId> {
        let athena_ir::TermNode::Application { arguments, .. } = session.arena.get(expr)?
        else {
            return None;
        };
        if arguments.len() != 2 {
            return None;
        }
        let athena_ir::TermNode::Application { arguments: inner, .. } = session.arena.get(arguments[0])?
        else {
            return None;
        };
        (inner.len() == 1).then_some(inner[0])
    };
    match (arg(a), arg(b)) {
        (Some(x), Some(y)) => session.arena.structural_eq(x, y),
        _ => false,
    }
}

fn nested_list_shape(session: &Session, term: TermId) -> Option<(u64, u64)> {
    let athena_ir::TermNode::List(rows) = session.arena.get(term)?
    else {
        return None;
    };
    if rows.is_empty() {
        return Some((0, 0));
    }
    if matches!(session.arena.get(rows[0]), Some(athena_ir::TermNode::List(_))) {
        let mut cols: Option<u64> = None;
        for row in rows {
            let cells = match session.arena.get(*row) {
                Some(athena_ir::TermNode::List(cells)) => cells.len() as u64,
                _ => return None,
            };
            match cols {
                Some(prev) if prev != cells => return None,
                None => cols = Some(cells),
                _ => {}
            }
        }
        Some((rows.len() as u64, cols.unwrap_or(0)))
    }
    else {
        Some((1, rows.len() as u64))
    }
}

fn term_scalar_rational_session(session: &Session, term: TermId) -> Option<Rational> {
    let n = number_of(session, term)?;
    if let Some(i) = n.as_exact_integer() {
        return Some(Rational::new(Integer::from_i64(i), Integer::one()));
    }
    if let Some(i) = n.as_integer() {
        return Some(Rational::new(clone_integer(i), Integer::one()));
    }
    n.as_rational().map(clone_rational)
}

fn term_to_rational_matrix_session(session: &Session, term: TermId) -> Option<MatrixValue> {
    match session.arena.get(term) {
        Some(athena_ir::TermNode::List(rows)) if !rows.is_empty() => {
            if matches!(session.arena.get(rows[0]), Some(athena_ir::TermNode::List(_))) {
                let mut data = Vec::new();
                let mut cols: Option<u64> = None;
                for row in rows {
                    let cells = match session.arena.get(*row) {
                        Some(athena_ir::TermNode::List(cells)) => cells.clone(),
                        _ => return None,
                    };
                    let c = cells.len() as u64;
                    match cols {
                        Some(prev) if prev != c => return None,
                        None => cols = Some(c),
                        _ => {}
                    }
                    for cell in cells {
                        data.push(term_scalar_rational_session(session, cell)?);
                    }
                }
                MatrixValue::from_rationals_row_major(rows.len() as u64, cols.unwrap_or(0), data).ok()
            }
            else {
                let mut data = Vec::with_capacity(rows.len());
                for cell in rows {
                    data.push(term_scalar_rational_session(session, *cell)?);
                }
                MatrixValue::from_rationals_row_major(1, data.len() as u64, data).ok()
            }
        }
        _ => {
            let r = term_scalar_rational_session(session, term)?;
            MatrixValue::from_rationals_row_major(1, 1, vec![r]).ok()
        }
    }
}

fn rational_to_term_session(session: &mut Session, r: &Rational) -> TermId {
    if r.is_integer() {
        if let Some(i) = r.numerator().to_i64() {
            return session.builder().int(i, Default::default());
        }
    }
    push_number(session, Number::from_rational_normalized(clone_rational(r)))
}

fn matrix_to_nested_list_session(session: &mut Session, m: &MatrixValue) -> Result<TermId> {
    let (rows, cols) = (m.shape().rows, m.shape().cols);
    let mut out = Vec::with_capacity(rows as usize);
    for i in 0..rows {
        let mut row = Vec::with_capacity(cols as usize);
        for j in 0..cols {
            match m.get(i, j)? {
                MatrixEntry::Rational(r) => row.push(rational_to_term_session(session, &r)),
                MatrixEntry::Integer(n) => {
                    if let Some(i64v) = n.to_i64() {
                        row.push(session.builder().int(i64v, Default::default()));
                    }
                    else {
                        row.push(push_number(session, Number::integer(clone_integer(&n))));
                    }
                }
                MatrixEntry::MachineF64(x) => row.push(push_number(session, Number::machine(x))),
            }
        }
        out.push(push_list(session, row));
    }
    Ok(push_list(session, out))
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
            Some(athena_ir::TermNode::List(items)) if items.len() == 2 => {
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
            Some(athena_ir::TermNode::List(items)) if items.len() == 2 => {
                assert_eq!(number_of(&session, items[0]).and_then(|n| n.as_exact_integer()), Some(2));
                assert_eq!(number_of(&session, items[1]).and_then(|n| n.as_exact_integer()), Some(4));
            }
            other => panic!("expected List[2, 4], got {other:?}"),
        }
    }
}
