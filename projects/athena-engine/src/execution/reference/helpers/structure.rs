//! 结构算子（`Join` / `Range`）的纯 term 折叠。

use athena_ir::SemanticOperator;
use athena_types::{Result, TermId};

use crate::{
    execution::{number_of, push_semantic},
    runtime::{session::Session, values::arena::push_list},
};

use super::terms::expand_span_3;

/// `Join[list…]` — 展平有序集合；任一非集合则残差。
pub(crate) fn evaluate_join_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    let mut out = Vec::new();
    for term in &terms {
        match session.arena.get(*term) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) => out.extend_from_slice(items),
            _ => return Ok(push_semantic(session, SemanticOperator::Join, terms)),
        }
    }
    Ok(push_list(session, out))
}

/// `Range[n]` / `Range[a,b]` / `Range[a,b,step]` — 精确整数展开；否则残差。
pub(crate) fn evaluate_range_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    let ints = terms
        .iter()
        .map(|t| number_of(session, *t).and_then(|n| n.as_exact_integer()))
        .collect::<Option<Vec<_>>>();
    let Some(ints) = ints else {
        return Ok(push_semantic(session, SemanticOperator::Range, terms));
    };
    let bounds = match ints.as_slice() {
        [n] => Some((1, *n, 1)),
        [a, b] => Some((*a, *b, 1)),
        [a, b, step] => Some((*a, *b, *step)),
        _ => None,
    };
    let Some((a, b, step)) = bounds else {
        return Ok(push_semantic(session, SemanticOperator::Range, terms));
    };
    let Some(values) = expand_span_3(a, step, b) else {
        return Ok(push_semantic(session, SemanticOperator::Range, terms));
    };
    let out: Vec<TermId> = values
        .into_iter()
        .map(|v| session.builder().int(v, Default::default()))
        .collect();
    Ok(push_list(session, out))
}
