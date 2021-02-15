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

pub(super) fn is_compare_operator(op: SemanticOperator) -> bool {
    matches!(
        op,
        SemanticOperator::Less
            | SemanticOperator::Greater
            | SemanticOperator::LessEqual
            | SemanticOperator::GreaterEqual
    )
}

/// 收集混合比较链：`Greater[Less[a,b],c]` → values `[a,b,c]` · ops `[Less, Greater]`。
pub(super) fn flatten_mixed_compare_chain(session: &Session, term: TermId) -> Option<(Vec<TermId>, Vec<SemanticOperator>)> {
    let mut values = Vec::new();
    let mut ops = Vec::new();
    if !collect_mixed_compare_chain(session, term, &mut values, &mut ops) {
        return None;
    }
    if ops.len() < 2 || values.len() != ops.len() + 1 {
        return None;
    }
    if ops.iter().all(|o| *o == ops[0]) {
        return None;
    }
    Some((values, ops))
}

fn collect_mixed_compare_chain(
    session: &Session,
    term: TermId,
    values: &mut Vec<TermId>,
    ops: &mut Vec<SemanticOperator>,
) -> bool {
    let Some(TermNode::Application { head, arguments }) = session.arena.get(term)
    else {
        return false;
    };
    let ApplicationHead::Semantic(op) = *head
    else {
        return false;
    };
    if !is_compare_operator(op) || arguments.len() != 2 {
        return false;
    }
    let left = arguments[0];
    let right = arguments[1];
    if collect_mixed_compare_chain(session, left, values, ops) {
        ops.push(op);
        values.push(right);
    }
    else {
        values.push(left);
        values.push(right);
        ops.push(op);
    }
    true
}

#[cfg(test)]
mod mixed_compare_chain_tests {
    use super::{flatten_mixed_compare_chain, is_compare_operator};
    use athena_ir::{ApplicationHead, SemanticOperator};
    use crate::runtime::{Session, values::arena::push_int};

    #[test]
    fn unpacks_greater_less_chain() {
        let mut s = Session::new();
        let one = push_int(&mut s, 1);
        let two = push_int(&mut s, 2);
        let three = push_int(&mut s, 3);
        let inner = s.builder().application(
            ApplicationHead::Semantic(SemanticOperator::Less),
            vec![one, three],
            Default::default(),
        );
        let term = s.builder().application(
            ApplicationHead::Semantic(SemanticOperator::Greater),
            vec![inner, two],
            Default::default(),
        );
        let (values, ops) = flatten_mixed_compare_chain(&s, term).expect("mixed chain");
        assert_eq!(values.len(), 3);
        assert_eq!(ops, vec![SemanticOperator::Less, SemanticOperator::Greater]);
        assert!(is_compare_operator(SemanticOperator::Less));
    }
}
