//! Symbolic arithmetic folding helpers.

use athena_numeric::{Number, add as num_add, mul as num_mul, pow as num_pow};
use athena_types::TermId;

use athena_ir::{ApplicationHead, SemanticOperator};

use crate::{
    execution::{number_of, push_number, push_semantic},
    runtime::{
        session::Session,
        values::{arena::push_list, numeric_clone::clone_number},
    },
};

fn is_sem(head: ApplicationHead, op: SemanticOperator) -> bool {
    matches!(head, ApplicationHead::Semantic(o) if o == op)
}

pub(crate) fn fold_plus_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
    // Flatten one level of nested `Plus` and coalesce numeric summands.
    let mut flat = Vec::with_capacity(terms.len());
    let mut sum: Option<Number> = None;
    for term in terms {
        match session.arena.get(term) {
            Some(athena_ir::TermNode::Application { head, arguments }) if is_sem(*head, SemanticOperator::Add) => {
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
        _ => push_semantic(session, SemanticOperator::Add, flat),
    }
}

pub(crate) fn push_plus_summand_session(session: &mut Session, term: TermId, flat: &mut Vec<TermId>, sum: &mut Option<Number>) {
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
pub(crate) fn combine_like_plus_session(session: &mut Session, terms: Vec<TermId>) -> Vec<TermId> {
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

pub(crate) fn split_numeric_coeff_session(session: &mut Session, term: TermId) -> (Number, TermId) {
    if let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(term) {
        if is_sem(*head, SemanticOperator::Multiply) && !arguments.is_empty() {
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
                _ => push_semantic(session, SemanticOperator::Multiply, rest),
            };
            return (coef, kernel);
        }
    }
    if let Some(n) = number_of(session, term) {
        return (clone_number(n), session.builder().int(1, Default::default()));
    }
    (Number::small_int(1), term)
}

pub(crate) fn groups_to_plus_terms_session(session: &mut Session, groups: Vec<(TermId, Number)>) -> Vec<TermId> {
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

pub(crate) fn fold_times_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
    // Flatten one level of nested `Times`.
    let mut flat = Vec::with_capacity(terms.len());
    for term in terms {
        match session.arena.get(term) {
            Some(athena_ir::TermNode::Application { head, arguments }) if is_sem(*head, SemanticOperator::Multiply) => {
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
                if is_sem(*head, SemanticOperator::Add)
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
        _ => push_semantic(session, SemanticOperator::Multiply, out),
    }
}

pub(crate) fn canonicalize_times_factors_session(session: &mut Session, factors: Vec<TermId>) -> Vec<TermId> {
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
pub(crate) fn combine_like_powers_session(session: &mut Session, factors: Vec<TermId>) -> Vec<TermId> {
    let mut groups: Vec<(TermId, TermId)> = Vec::new();
    let mut rest = Vec::new();
    for f in factors {
        let base_exp = match session.arena.get(f) {
            Some(athena_ir::TermNode::Application { head, arguments }) if is_sem(*head, SemanticOperator::Power) && arguments.len() == 2 => {
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
                                Err(_) => push_semantic(session, SemanticOperator::Add, vec![*e, exp]),
                            },
                            _ => push_semantic(session, SemanticOperator::Add, vec![*e, exp]),
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

pub(crate) fn fold_divide_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
    if terms.len() != 2 {
        return push_semantic(session, SemanticOperator::Divide, terms);
    }
    let (num, den) = (terms[0], terms[1]);
    let neg1 = session.builder().int(-1, Default::default());
    let inv = fold_power_symbolic(session, vec![den, neg1]);
    fold_times_symbolic(session, vec![num, inv])
}

pub(crate) fn fold_subtract_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
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
        _ => push_semantic(session, SemanticOperator::Subtract, terms),
    }
}

pub(crate) fn fold_power_symbolic(session: &mut Session, terms: Vec<TermId>) -> TermId {
    if terms.len() != 2 {
        return push_semantic(session, SemanticOperator::Power, terms);
    }
    let (base, exp) = (terms[0], terms[1]);
    if let Some(e) = number_of(session, exp) {
        if e.is_zero() {
            // Scalar `x^0 → 1`; list bases stay residual (elementwise is `DotPower`).
            if matches!(session.arena.get(base), Some(athena_ir::TermNode::Collection { elements: _, .. })) {
                return push_semantic(session, SemanticOperator::Power, terms);
            }
            return session.builder().int(1, Default::default());
        }
        if e.is_one() {
            return base;
        }
        // `(u^a)^b → u^(a*b)` and `(c*u)^n → c^n * u^n` when exponents are integers.
        if e.as_integer_exp().is_some() {
            if let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(base) {
                let head = *head;
                if is_sem(head, SemanticOperator::Power) && arguments.len() == 2 {
                    let inner_base = arguments[0];
                    if let Some(inner_exp) = number_of(session, arguments[1]) {
                        if let Ok(combined) = num_mul(clone_number(inner_exp), clone_number(e)) {
                            let combined_id = push_number(session, combined);
                            return fold_power_symbolic(session, vec![inner_base, combined_id]);
                        }
                    }
                }
                if is_sem(head, SemanticOperator::Multiply) && arguments.len() >= 2 {
                    let args = arguments.clone();
                    if let Some(c) = number_of(session, args[0]) {
                        if let Ok(cp) = num_pow(c, e) {
                            let rest =
                                if args.len() == 2 { args[1] } else { push_semantic(session, SemanticOperator::Multiply, args[1..].to_vec()) };
                            let rest_pow = fold_power_symbolic(session, vec![rest, exp]);
                            let cp_id = push_number(session, cp);
                            return fold_times_symbolic(session, vec![cp_id, rest_pow]);
                        }
                    }
                }
            }
        }
    }
    push_semantic(session, SemanticOperator::Power, terms)
}
