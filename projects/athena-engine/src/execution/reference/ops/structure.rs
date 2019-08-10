//! List / matrix / arithmetic operator evaluation.

use std::cmp::Ordering;
use std::collections::HashMap;

use athena_numeric::{
    Number, add as num_add, compare as num_compare, div as num_div, mul as num_mul, pow as num_pow,
};
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

use super::super::{IndexStep, ReferenceExecutor, Slot};
use super::super::helpers::*;
use crate::{
    api::request::AthenaRequest,
    domains::linear_algebra::{SolveDisposition, det_bareiss, solve_exact},
    execution::{compiler::ExecutionCompiler, ir::SsaValueId, number_of, push_application, push_number},
    runtime::{
        session::Session,
        values::{arena::push_list, numeric_clone::{clone_number, clone_rational}},
    },
};

impl ReferenceExecutor {
    /// `Sum[list]` — vector scalar sum / matrix column sums.
    /// `Sum[body, iterator]` — expand iterator then Plus-fold.
    pub(crate) fn eval_sum(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
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
        let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(term)
        else {
            return Ok(Slot::Term(push_application(session, "Sum", vec![term])));
        };
        let items = items.clone();
        if items.is_empty() {
            return Ok(Slot::Term(session.builder().int(0, Default::default())));
        }
        if matches!(session.arena.get(items[0]), Some(athena_ir::TermNode::Collection { elements: _, .. })) {
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
                        Some(athena_ir::TermNode::Collection { elements: cells, .. }) => cells.get(j).copied(),
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

    pub(crate) fn table_values(&self, session: &mut Session, body: TermId, iter: TermId) -> Result<Option<Vec<TermId>>> {
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

    pub(crate) fn eval_det(
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

    pub(crate) fn eval_linear_solve(
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

    pub(crate) fn eval_range(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
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

    /// Execute neutral [`IndexSpec`] axes against a target SSA value.
    pub(crate) fn eval_index(
        &self,
        session: &mut Session,
        target: SsaValueId,
        axes: &[athena_types::IndexSpec],
        slots: &HashMap<SsaValueId, Slot>,
        invalid: &mut Option<Diagnostic>,
    ) -> Result<Slot> {
        use athena_types::IndexSpec;

        let slot = *slots.get(&target).ok_or_else(|| diag("index_target_undefined"))?;
        let mut cur = self.slot_as_term(session, slot)?;

        // `All` then remaining axes over each row (column / nested selection).
        if let [IndexSpec::All, rest @ ..] = axes {
            if !rest.is_empty() {
                if let Some(athena_ir::TermNode::Collection { elements: rows, .. }) = session.arena.get(cur) {
                    let rows = rows.clone();
                    let mut out = Vec::with_capacity(rows.len());
                    for row in rows {
                        let mut cell = row;
                        for axis in rest {
                            match self.index_one(session, cell, axis)? {
                                IndexStep::Next(next) => cell = next,
                                IndexStep::Residual => return Ok(Slot::Term(cur)),
                                IndexStep::Invalid { echo, diagnostic } => {
                                    *invalid = Some(diagnostic);
                                    return Ok(Slot::Term(echo));
                                }
                            }
                        }
                        out.push(cell);
                    }
                    return Ok(Slot::Term(push_list(session, out)));
                }
            }
        }

        for axis in axes {
            match self.index_one(session, cur, axis)? {
                IndexStep::Next(next) => cur = next,
                IndexStep::Residual => return Ok(Slot::Term(cur)),
                IndexStep::Invalid { echo, diagnostic } => {
                    *invalid = Some(diagnostic);
                    return Ok(Slot::Term(echo));
                }
            }
        }
        Ok(Slot::Term(cur))
    }

    /// Apply one [`IndexSpec`] axis (1-based scalar, All, EndRelative, Range).
    pub(crate) fn index_one(&self, session: &mut Session, expr: TermId, spec: &athena_types::IndexSpec) -> Result<IndexStep> {
        use athena_types::{IndexSpec, IntegerIndex, IntegerOffset};

        let items = match session.arena.get(expr) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
            Some(athena_ir::TermNode::Application { arguments, .. }) => arguments.clone(),
            _ => return Ok(IndexStep::Residual),
        };
        let len = items.len();

        match spec {
            IndexSpec::All => Ok(IndexStep::Next(push_list(session, items))),
            IndexSpec::EndRelative(IntegerOffset(off)) => {
                let pos = len as i64 + *off - 1;
                if pos < 0 || pos as usize >= len {
                    return Ok(IndexStep::Invalid {
                        echo: expr,
                        diagnostic: crate::diagnostics::invalid_index_diagnostic(*off, Some(len as u64)),
                    });
                }
                Ok(IndexStep::Next(items[pos as usize]))
            }
            IndexSpec::Scalar(IntegerIndex(idx)) => {
                if *idx == 0 {
                    return Ok(IndexStep::Next(match session.arena.get(expr) {
                        Some(athena_ir::TermNode::Collection { .. }) => {
                            session.builder().symbol("OrderedCollection", Default::default())
                        }
                        Some(athena_ir::TermNode::Application { head, .. }) => {
                            let name = session.operators.name(*head).unwrap_or("").to_string();
                            session.builder().symbol(&name, Default::default())
                        }
                        _ => return Ok(IndexStep::Residual),
                    }));
                }
                let pos = if *idx > 0 {
                    (*idx - 1) as usize
                }
                else {
                    let pos = len as i64 + *idx;
                    if pos < 0 {
                        return Ok(IndexStep::Invalid {
                            echo: expr,
                            diagnostic: crate::diagnostics::invalid_index_diagnostic(*idx, Some(len as u64)),
                        });
                    }
                    pos as usize
                };
                match items.get(pos) {
                    Some(item) => Ok(IndexStep::Next(*item)),
                    None => Ok(IndexStep::Invalid {
                        echo: expr,
                        diagnostic: crate::diagnostics::invalid_index_diagnostic(*idx, Some(len as u64)),
                    }),
                }
            }
            IndexSpec::Range { start, end, step } => {
                let Some(values) = expand_span_3(start.0, *step, end.0)
                else {
                    return Ok(IndexStep::Residual);
                };
                let mut out = Vec::with_capacity(values.len());
                for v in values {
                    match self.index_one(session, expr, &IndexSpec::Scalar(IntegerIndex(v)))? {
                        IndexStep::Next(item) => out.push(item),
                        IndexStep::Residual => return Ok(IndexStep::Residual),
                        IndexStep::Invalid { echo, diagnostic } => {
                            return Ok(IndexStep::Invalid { echo, diagnostic });
                        }
                    }
                }
                Ok(IndexStep::Next(push_list(session, out)))
            }
            IndexSpec::Cartesian(axes) => {
                let mut cur = expr;
                for axis in axes {
                    match self.index_one(session, cur, axis)? {
                        IndexStep::Next(next) => cur = next,
                        other => return Ok(other),
                    }
                }
                Ok(IndexStep::Next(cur))
            }
            IndexSpec::DomainSpecific(_) => Ok(IndexStep::Residual),
        }
    }

    pub(crate) fn eval_join(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        let mut out = Vec::new();
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            let term = self.slot_as_term(session, slot)?;
            terms.push(term);
            match session.arena.get(term) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) => out.extend_from_slice(items),
                _ => return Ok(Slot::Term(push_application(session, "Join", terms))),
            }
        }
        Ok(Slot::Term(push_list(session, out)))
    }

    pub(crate) fn eval_compare_chain(&self, session: &mut Session, name: &str, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
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
        // Binary list broadcast for compares.
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

    pub(crate) fn eval_arithmetic(&self, session: &mut Session, name: &str, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
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

    pub(crate) fn slot_as_term(&self, session: &mut Session, slot: Slot) -> Result<TermId> {
        match slot {
            Slot::Term(term) => Ok(term),
            Slot::Boolean(value) => Ok(session.builder().boolean(value, Default::default())),
            Slot::Symbol(symbol) => Ok(session.builder().symbol_id(symbol, Default::default())),
            Slot::Unit => Ok(session.builder().null(Default::default())),
            Slot::Scope(_) | Slot::Result(_) => Err(diag("slot_not_term")),
        }
    }
}
