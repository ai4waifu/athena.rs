//! 多项式结构算子（`Coefficient` / `Exponent`）的窄 exact 折叠。

use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode};
use athena_numeric::{add as num_add, Number};
use athena_types::{Result, TermId};

use super::{arithmetic::split_numeric_coeff_session, diag, terms::symbol_name};
use crate::{
    execution::{number_of, push_number, push_semantic},
    runtime::{
        session::Session,
        values::{arena::push_int, numeric_clone::clone_number},
    },
};

/// `Coefficient[expr, var]` — 默认求 `var^1` 项系数之和。
pub(crate) fn evaluate_coefficient_terms(session: &mut Session, expr: TermId, var: TermId) -> Result<TermId> {
    if symbol_name(session, var).is_none() {
        return Ok(push_semantic(session, SemanticOperator::Coefficient, vec![expr, var]));
    }
    let mut sum = Number::small_int(0);
    let mut summands = Vec::new();
    collect_add_summands(session, expr, &mut summands);
    for term in summands {
        let (coef, kernel) = split_numeric_coeff_session(session, term);
        match kernel_exponent_in_var(session, kernel, var) {
            Some(1) => {
                sum = num_add(clone_number(&sum), clone_number(&coef)).unwrap_or(sum);
            }
            Some(_) => {}
            None => return Ok(push_semantic(session, SemanticOperator::Coefficient, vec![expr, var])),
        }
    }
    Ok(push_number(session, sum))
}

/// `Exponent[expr, var]` — `var` 在 `expr` 中的最大指数。
pub(crate) fn evaluate_exponent_terms(session: &mut Session, expr: TermId, var: TermId) -> Result<TermId> {
    if symbol_name(session, var).is_none() {
        return Ok(push_semantic(session, SemanticOperator::Exponent, vec![expr, var]));
    }
    let mut max_exp = 0i64;
    let mut summands = Vec::new();
    collect_add_summands(session, expr, &mut summands);
    for term in summands {
        let (coef, kernel) = split_numeric_coeff_session(session, term);
        if coef.is_zero() {
            continue;
        }
        match kernel_exponent_in_var(session, kernel, var) {
            Some(exp) if exp > max_exp => max_exp = exp,
            Some(_) => {}
            None => return Ok(push_semantic(session, SemanticOperator::Exponent, vec![expr, var])),
        }
    }
    Ok(push_int(session, max_exp))
}

fn collect_add_summands(session: &Session, expr: TermId, out: &mut Vec<TermId>) {
    match session.arena.get(expr) {
        Some(TermNode::Application { head, arguments }) if matches!(head, ApplicationHead::Semantic(SemanticOperator::Add)) => {
            for arg in arguments {
                collect_add_summands(session, *arg, out);
            }
        }
        _ => out.push(expr),
    }
}

fn kernel_exponent_in_var(session: &Session, kernel: TermId, var: TermId) -> Option<i64> {
    if session.arena.structural_eq(kernel, var) {
        return Some(1);
    }
    match session.arena.get(kernel) {
        Some(TermNode::Application { head, arguments }) => {
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Power)) && arguments.len() == 2 {
                if session.arena.structural_eq(arguments[0], var) {
                    return number_of(session, arguments[1]).and_then(|n| n.as_exact_integer());
                }
                return if term_depends_on_var(session, kernel, var) {
                    None
                }
                else {
                    Some(0)
                };
            }
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Multiply)) {
                let mut total = 0i64;
                for arg in arguments {
                    total = total.checked_add(kernel_exponent_in_var(session, *arg, var)?)?;
                }
                return Some(total);
            }
            if term_depends_on_var(session, kernel, var) {
                return None;
            }
            Some(0)
        }
        Some(TermNode::Atom(Atom::Symbol(_))) => Some(0),
        _ => {
            if term_depends_on_var(session, kernel, var) {
                None
            }
            else {
                Some(0)
            }
        }
    }
}

fn term_depends_on_var(session: &Session, term: TermId, var: TermId) -> bool {
    if session.arena.structural_eq(term, var) {
        return true;
    }
    match session.arena.get(term) {
        Some(TermNode::Application { arguments, .. }) => arguments.iter().any(|arg| term_depends_on_var(session, *arg, var)),
        Some(TermNode::Collection { elements, .. }) => elements.iter().any(|arg| term_depends_on_var(session, *arg, var)),
        _ => false,
    }
}
