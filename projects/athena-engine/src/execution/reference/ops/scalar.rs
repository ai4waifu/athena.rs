//! Scalar / rewrite / pattern operator evaluation.

use std::collections::HashMap;

use athena_numeric::{
    Number, abs as num_abs, factorial as num_factorial, sqrt as num_sqrt, to_f64_lossy as num_to_f64_lossy,
};
use athena_ir::{ApplicationHead, SemanticOperator, UnaryFunction};
use athena_types::{Result, SymbolId, TermId};

use super::super::{ReferenceExecutor, Slot};
use super::super::helpers::*;
use crate::{
    api::request::AthenaRequest,
    execution::{compiler::ExecutionCompiler, ir::SsaValueId, number_of, push_extension, push_number, push_semantic},
    runtime::{session::Session, values::arena::push_list, values::numeric_clone::clone_number},
};

impl ReferenceExecutor {
    pub(crate) fn eval_unary_term_op(&self, session: &mut Session, op: SemanticOperator, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 1 {
            return Err(diag("semantic_operator_arity"));
        }
        let slot = *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?;
        let term = self.slot_as_term(session, slot)?;
        match op {
            SemanticOperator::Abs => {
                if let Some(n) = number_of(session, term) {
                    Ok(Slot::Term(push_number(session, num_abs(clone_number(n)))))
                }
                else {
                    Ok(Slot::Term(push_semantic(session, SemanticOperator::Abs, vec![term])))
                }
            }
            SemanticOperator::Factorial => {
                if let Some(n) = number_of(session, term) {
                    match num_factorial(n) {
                        Ok(v) => Ok(Slot::Term(push_number(session, v))),
                        Err(_) => Ok(Slot::Term(push_semantic(session, SemanticOperator::Factorial, vec![term]))),
                    }
                }
                else {
                    Ok(Slot::Term(push_semantic(session, SemanticOperator::Factorial, vec![term])))
                }
            }
            SemanticOperator::Sqrt => {
                if let Some(n) = number_of(session, term) {
                    match num_sqrt(n) {
                        Ok(Some(v)) => Ok(Slot::Term(push_number(session, v))),
                        _ => Ok(Slot::Term(push_semantic(session, SemanticOperator::Sqrt, vec![term]))),
                    }
                }
                else {
                    Ok(Slot::Term(push_semantic(session, SemanticOperator::Sqrt, vec![term])))
                }
            }
            SemanticOperator::Length => {
                let len = match session.arena.get(term) {
                    Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.len() as i64,
                    Some(athena_ir::TermNode::Application { arguments, .. }) => arguments.len() as i64,
                    _ => return Ok(Slot::Term(push_semantic(session, SemanticOperator::Length, vec![term]))),
                };
                Ok(Slot::Term(session.builder().int(len, Default::default())))
            }
            SemanticOperator::First => match session.arena.get(term) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) if !items.is_empty() => Ok(Slot::Term(items[0])),
                Some(athena_ir::TermNode::Application { arguments, .. }) if !arguments.is_empty() => Ok(Slot::Term(arguments[0])),
                Some(athena_ir::TermNode::Collection { elements: _, .. } | athena_ir::TermNode::Application { .. }) => Err(diag("first_empty")),
                _ => Ok(Slot::Term(push_semantic(session, SemanticOperator::First, vec![term]))),
            },
            SemanticOperator::Rest => match session.arena.get(term) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) if !items.is_empty() => {
                    let rest = items[1..].to_vec();
                    Ok(Slot::Term(push_list(session, rest)))
                }
                Some(athena_ir::TermNode::Application { head, arguments }) if !arguments.is_empty() => {
                    let head = *head;
                    let rest = arguments[1..].to_vec();
                    Ok(Slot::Term(session.builder().application(head, rest, Default::default())))
                }
                Some(athena_ir::TermNode::Collection { elements: _, .. } | athena_ir::TermNode::Application { .. }) => Err(diag("rest_empty")),
                _ => Ok(Slot::Term(push_semantic(session, SemanticOperator::Rest, vec![term]))),
            },
            _ => Err(diag("semantic_operator_not_implemented")),
        }
    }

    pub(crate) fn eval_residual_app(&self, session: &mut Session, name: &str, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        // Extension residuals only — core trig/specials evaluate via SemanticOperator::Unary.
        let op = session.operators.intern(name);
        Ok(Slot::Term(push_extension(session, op, terms)))
    }

    /// Apply the first matching Session dispatch rule and re-evaluate the replacement.

    pub(crate) fn eval_residual_semantic(
        &self,
        session: &mut Session,
        op: SemanticOperator,
        args: &[SsaValueId],
        slots: &HashMap<SsaValueId, Slot>,
    ) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        if let Some(uf) = op.as_unary() {
            if terms.len() == 1 {
                let arg = terms[0];
                if let Some(exact) = eval_trig_exact_session(session, uf, arg) {
                    return Ok(Slot::Term(exact));
                }
                if let Some(x) = term_as_f64_session(session, arg) {
                    let y = match uf {
                        UnaryFunction::Sin => x.sin(),
                        UnaryFunction::Cos => x.cos(),
                        UnaryFunction::Tan => x.tan(),
                        UnaryFunction::Exp => x.exp(),
                        UnaryFunction::Log => x.ln(),
                        _ => f64::NAN,
                    };
                    if y.is_finite() {
                        return Ok(Slot::Term(push_number(session, Number::machine(y))));
                    }
                }
            }
        }
        Ok(Slot::Term(push_semantic(session, op, terms)))
    }

    pub(crate) fn try_apply_down_values(
        &self,
        session: &mut Session,
        name: &str,
        args: &[SsaValueId],
        slots: &HashMap<SsaValueId, Slot>,
    ) -> Result<Option<Slot>> {
        let symbol = session.arena.symbols_mut().intern(name);
        let Some(rules) = session.defs.dispatch_rules(symbol).map(|r| r.to_vec())
        else {
            return Ok(None);
        };
        let call_op = ApplicationHead::Extension(session.operators.intern(name));
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = *slots.get(id).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let substituted = {
            let mut matched = None;
            for (pattern, rhs) in rules {
                let mut binds = HashMap::new();
                let ok = match &pattern {
                    crate::reasoning::trs::TermPattern::Application { operator, arguments } => {
                        *operator == call_op && arguments.len() == terms.len() && {
                            arguments.iter().zip(terms.iter()).all(|(p, a)| {
                                crate::execution::builtins::patterns::match_term_pattern(session, *a, p, &mut binds)
                            })
                        }
                    }
                    crate::reasoning::trs::TermPattern::StructuralApplication(arguments) => {
                        arguments.len() == terms.len()
                            && arguments.iter().zip(terms.iter()).all(|(p, a)| {
                                crate::execution::builtins::patterns::match_term_pattern(session, *a, p, &mut binds)
                            })
                    }
                    _ => false,
                };
                if ok {
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

    pub(crate) fn eval_simplify(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
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

    pub(crate) fn eval_rule(&self, session: &mut Session, op: SemanticOperator, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let lhs = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let rhs = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        Ok(Slot::Term(push_semantic(session, op, vec![lhs, rhs])))
    }

    pub(crate) fn eval_replace_all(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let expr = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let rules_term = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let rules = collect_rule_pairs(session, rules_term);
        if rules.is_empty() {
            return Ok(Slot::Term(push_semantic(session, SemanticOperator::ReplaceAll, vec![expr, rules_term])));
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

    /// `CollectMatches[list, pat]` — filter list items by pattern.
    pub(crate) fn eval_collect_matches(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let list = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let pat = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(list)
        else {
            return Ok(Slot::Term(push_semantic(session, SemanticOperator::CollectMatches, vec![list, pat])));
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
    pub(crate) fn eval_matches(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let expr = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let pat = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let matched = crate::execution::builtins::patterns::pattern_matches(session, expr, pat);
        Ok(Slot::Boolean(matched))
    }

    pub(crate) fn eval_matrix_constructor(
        &self,
        session: &mut Session,
        op: SemanticOperator,
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
            return Ok(Slot::Term(push_semantic(session, op, terms)));
        };
        let n = match rows.checked_mul(cols) {
            Some(v) if v <= 4096 => v as usize,
            _ => return Ok(Slot::Term(push_semantic(session, op, terms))),
        };
        if n == 0 {
            return Ok(Slot::Term(push_list(session, Vec::new())));
        }
        let fill = match op {
            SemanticOperator::Ones => 1i64,
            SemanticOperator::Zeros | SemanticOperator::Eye => 0,
            _ => return Err(diag("semantic_operator_not_implemented")),
        };
        let mut rows_out = Vec::with_capacity(rows as usize);
        for r in 0..rows {
            let mut row = Vec::with_capacity(cols as usize);
            for c in 0..cols {
                let value = if op == SemanticOperator::Eye && r == c { 1 } else { fill };
                row.push(session.builder().int(value, Default::default()));
            }
            rows_out.push(push_list(session, row));
        }
        Ok(Slot::Term(push_list(session, rows_out)))
    }

    pub(crate) fn eval_map(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let func = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let list = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let items = match session.arena.get(list) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
            _ => return Ok(Slot::Term(push_semantic(session, SemanticOperator::Map, vec![func, list]))),
        };
        if !self.map_func_supported(session, func) {
            return Ok(Slot::Term(push_semantic(session, SemanticOperator::Map, vec![func, list])));
        }
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(self.map_apply_one(session, func, item)?);
        }
        Ok(Slot::Term(push_list(session, out)))
    }

    fn map_func_supported(&self, session: &Session, func: TermId) -> bool {
        if symbol_name(session, func).is_some() {
            return true;
        }
        match session.arena.get(func) {
            Some(athena_ir::TermNode::Application {
                head: ApplicationHead::Semantic(SemanticOperator::Function),
                arguments,
            }) if arguments.len() == 2 => true,
            Some(athena_ir::TermNode::Application {
                head: ApplicationHead::Semantic(_) | ApplicationHead::Extension(_),
                arguments,
            }) if arguments.is_empty() => true,
            _ => false,
        }
    }

    /// Apply `func` to one list element: 0-ary operator value, symbol head, or `Function[var, body]`.
    fn map_apply_one(&self, session: &mut Session, func: TermId, item: TermId) -> Result<TermId> {
        if let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(func) {
            if arguments.is_empty() {
                let mapped = match *head {
                    ApplicationHead::Semantic(op) => push_semantic(session, op, vec![item]),
                    ApplicationHead::Extension(id) => {
                        let mut b = athena_ir::TermBuilder::new(&mut session.arena);
                        b.application_extension_id(id, vec![item], athena_ir::TermNode::default_span())
                    }
                };
                return self.re_eval_term(session, mapped);
            }
            if matches!(*head, ApplicationHead::Semantic(SemanticOperator::Function)) {
                let arguments = arguments.clone();
                if let [var, body] = arguments.as_slice() {
                    if let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(sym))) = session.arena.get(*var) {
                        let instantiated = crate::execution::builtins::patterns::substitute_symbol(session, *body, *sym, item);
                        return self.re_eval_term(session, instantiated);
                    }
                }
            }
        }
        if let Some(name) = symbol_name(session, func) {
            let op = session.operators.intern(&name);
            let mapped = push_extension(session, op, vec![item]);
            return self.re_eval_term(session, mapped);
        }
        Err(diag("map_func_unsupported"))
    }

    fn re_eval_term(&self, session: &mut Session, term: TermId) -> Result<TermId> {
        match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(term)) {
            Ok(module) => {
                let result_id = self.execute(session, &module, None)?;
                Ok(session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(term))
            }
            Err(_) => Ok(term),
        }
    }

    pub(crate) fn eval_apply(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let head = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let second = self.slot_as_term(session, *slots.get(&args[1]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let items = match session.arena.get(second) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
            _ => return Ok(Slot::Term(push_semantic(session, SemanticOperator::Apply, vec![head, second]))),
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
    pub(crate) fn eval_application_form(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
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
        // Pure Function[body] requires AnonymousArgument from dialect lowering (not string Slot).
        if let Some(athena_ir::TermNode::Application { head: op, arguments }) = session.arena.get(head) {
            if matches!(*op, ApplicationHead::Semantic(SemanticOperator::Function)) && call_args.len() == 1 {
                let arguments = arguments.clone();
                if let [var, body] = arguments.as_slice() {
                    if let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(sym))) = session.arena.get(*var) {
                        let sym = *sym;
                        let instantiated = crate::execution::builtins::patterns::substitute_symbol(session, *body, sym, call_args[0]);
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
        }
        if let Some(name) = symbol_name(session, head) {
            let op = session.operators.intern(&name);
            let app = push_extension(session, op, call_args);
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
        Ok(Slot::Term(push_semantic(session, SemanticOperator::ApplyHead, wrapped)))
    }

    pub(crate) fn eval_size(&self, session: &mut Session, args: &[SsaValueId], slots: &HashMap<SsaValueId, Slot>) -> Result<Slot> {
        if args.len() != 1 {
            return Err(diag("semantic_operator_arity"));
        }
        let term = self.slot_as_term(session, *slots.get(&args[0]).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let Some((rows, cols)) = nested_list_shape(session, term)
        else {
            return Ok(Slot::Term(push_semantic(session, SemanticOperator::Size, vec![term])));
        };
        let r = session.builder().int(rows as i64, Default::default());
        let c = session.builder().int(cols as i64, Default::default());
        Ok(Slot::Term(push_list(session, vec![r, c])))
    }
}
