//! Compare / boolean helpers for the reference executor.

use std::cmp::Ordering;

use athena_numeric::{Number, compare as num_compare};
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

use super::diag;
use super::super::Slot;
use crate::{
    execution::{number_of, push_application, push_number},
    runtime::{session::Session, values::arena::push_list, values::numeric_clone::clone_number},
};

pub(crate) fn compare_list_broadcast(
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

pub(crate) fn compare_pair_term(session: &mut Session, name: &str, left: TermId, right: TermId, pick: fn(Ordering) -> bool) -> Result<TermId> {
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

pub(crate) fn is_known_residual_head(name: &str) -> bool {
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
pub(crate) fn slot_as_boolean_like(session: &Session, slot: Slot) -> Option<bool> {
    match slot {
        Slot::Boolean(v) => Some(v),
        Slot::Term(term) => as_boolean_like_term(session, term),
        _ => None,
    }
}

pub(crate) fn as_boolean_like_term(session: &Session, term: TermId) -> Option<bool> {
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

pub(crate) fn coerce_branch_predicate(session: &Session, term: TermId) -> Result<bool> {
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
