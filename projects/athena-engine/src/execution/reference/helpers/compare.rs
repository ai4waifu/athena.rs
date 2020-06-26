//! Reference 执行器的比较 / 布尔辅助。

use std::cmp::Ordering;

use athena_ir::SemanticOperator;
use athena_numeric::{Number, compare as num_compare};
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

use super::{super::Slot, diag};
use crate::{
    execution::{number_of, push_number, push_semantic},
    runtime::{
        session::Session,
        values::{arena::push_list, numeric_clone::clone_number},
    },
};

pub(crate) fn compare_list_broadcast(
    session: &mut Session,
    op: SemanticOperator,
    left: TermId,
    right: TermId,
    pick: fn(Ordering) -> bool,
) -> Result<Option<TermId>> {
    let l_list = matches!(session.arena.get(left), Some(athena_ir::TermNode::Collection { elements: _, .. }));
    let r_list = matches!(session.arena.get(right), Some(athena_ir::TermNode::Collection { elements: _, .. }));
    match (l_list, r_list) {
        (false, false) => Ok(None),
        (true, true) => {
            let xs = match session.arena.get(left) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
                _ => return Ok(None),
            };
            let ys = match session.arena.get(right) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
                _ => return Ok(None),
            };
            if xs.len() != ys.len() {
                return Ok(Some(push_semantic(session, op, vec![left, right])));
            }
            let mut out = Vec::with_capacity(xs.len());
            for (a, b) in xs.into_iter().zip(ys.into_iter()) {
                out.push(compare_pair_term(session, op, a, b, pick)?);
            }
            Ok(Some(push_list(session, out)))
        }
        (true, false) => {
            let xs = match session.arena.get(left) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
                _ => return Ok(None),
            };
            let mut out = Vec::with_capacity(xs.len());
            for a in xs {
                out.push(compare_pair_term(session, op, a, right, pick)?);
            }
            Ok(Some(push_list(session, out)))
        }
        (false, true) => {
            let ys = match session.arena.get(right) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
                _ => return Ok(None),
            };
            let mut out = Vec::with_capacity(ys.len());
            for b in ys {
                out.push(compare_pair_term(session, op, left, b, pick)?);
            }
            Ok(Some(push_list(session, out)))
        }
    }
}

pub(crate) fn compare_pair_term(
    session: &mut Session,
    op: SemanticOperator,
    left: TermId,
    right: TermId,
    pick: fn(Ordering) -> bool,
) -> Result<TermId> {
    // 嵌套列表经广播递归。
    if matches!(session.arena.get(left), Some(athena_ir::TermNode::Collection { elements: _, .. }))
        || matches!(session.arena.get(right), Some(athena_ir::TermNode::Collection { elements: _, .. }))
    {
        return Ok(compare_list_broadcast(session, op, left, right, pick)?.unwrap_or_else(|| push_semantic(session, op, vec![left, right])));
    }
    match (number_of(session, left).map(clone_number), number_of(session, right).map(clone_number)) {
        (Some(a), Some(b)) => {
            let ord = num_compare(&a, &b).ok_or_else(|| diag("compare_failed"))?;
            Ok(session.builder().boolean(pick(ord), Default::default()))
        }
        _ => Ok(push_semantic(session, op, vec![left, right])),
    }
}

/// Reference 与 `ExecutionHost` 共用的比较链求值。
///
/// 全为数值时返回 Boolean 槽语义（`Ok(Either::Left)`）；否则返回 Term（广播 / 残差）。
pub(crate) fn evaluate_compare_terms(session: &mut Session, op: SemanticOperator, terms: Vec<TermId>) -> Result<CompareOutcome> {
    if terms.len() < 2 {
        return Err(diag("semantic_operator_arity"));
    }
    let pick = match op {
        SemanticOperator::Less => |o: Ordering| o == Ordering::Less,
        SemanticOperator::Greater => |o: Ordering| o == Ordering::Greater,
        SemanticOperator::LessEqual => |o: Ordering| o != Ordering::Greater,
        SemanticOperator::GreaterEqual => |o: Ordering| o != Ordering::Less,
        _ => return Err(diag("semantic_operator_not_implemented")),
    };
    if terms.len() == 2 {
        if let Some(broadcast) = compare_list_broadcast(session, op, terms[0], terms[1], pick)? {
            return Ok(CompareOutcome::Term(broadcast));
        }
    }
    let numbers = terms.iter().map(|t| number_of(session, *t).map(clone_number)).collect::<Option<Vec<_>>>();
    let Some(nums) = numbers
    else {
        return Ok(CompareOutcome::Term(push_semantic(session, op, terms)));
    };
    let mut ok = true;
    for window in nums.windows(2) {
        let ord = num_compare(&window[0], &window[1]).ok_or_else(|| diag("compare_failed"))?;
        if !pick(ord) {
            ok = false;
            break;
        }
    }
    Ok(CompareOutcome::Boolean(ok))
}

/// 比较求值结果（数值链 → Boolean；广播 / 残差 → Term）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompareOutcome {
    Boolean(bool),
    Term(TermId),
}

/// 逻辑运算：Boolean 原子 · `True`/`False` · 精确 `0`/`1`。
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
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) => Ok(!n.is_zero()),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Null)) => Ok(false),
        _ => Err(Diagnostic::new(DiagnosticCode::NonBooleanCondition)
            .detail("component", "ReferenceExecutor")
            .detail("reason", "branch_term_not_boolean")),
    }
}
