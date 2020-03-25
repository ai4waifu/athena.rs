//! Lowering 比较 / span 形式时用的纯辅助函数。

use athena_ir::{ApplicationHead, SemanticOperator, TermNode};
use athena_types::TermId;

use crate::runtime::session::Session;

pub(super) fn expand_span_range(start: i64, step: i64, end: i64) -> Option<Vec<i64>> {
    if step == 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut cur = start;
    if step > 0 {
        while cur <= end {
            out.push(cur);
            cur = cur.checked_add(step)?;
        }
    }
    else {
        while cur >= end {
            out.push(cur);
            cur = cur.checked_add(step)?;
        }
    }
    Some(out)
}

/// 收集左嵌套比较操作数：`Less[Less[a,b],c]` → `[a,b,c]`.
pub(super) fn flatten_compare_chain_args(session: &Session, op: SemanticOperator, term: TermId) -> Option<Vec<TermId>> {
    let mut out = Vec::new();
    if !collect_compare_chain_args(session, op, term, &mut out) {
        return None;
    }
    if out.len() < 2 {
        return None;
    }
    Some(out)
}

pub(super) fn collect_compare_chain_args(session: &Session, op: SemanticOperator, term: TermId, out: &mut Vec<TermId>) -> bool {
    let Some(TermNode::Application { head, arguments }) = session.arena.get(term)
    else {
        return false;
    };
    if !matches!(*head, ApplicationHead::Semantic(h) if h == op) || arguments.len() != 2 {
        return false;
    }
    let left = arguments[0];
    let right = arguments[1];
    if !collect_compare_chain_args(session, op, left, out) {
        out.push(left);
    }
    out.push(right);
    true
}
