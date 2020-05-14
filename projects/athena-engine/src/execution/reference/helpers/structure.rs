//! 结构算子（`Join` / `Range` / `Size` / `Sum`）的纯 term 折叠。

use athena_ir::SemanticOperator;
use athena_types::{Result, TermId};

use crate::{
    execution::{number_of, push_semantic},
    runtime::{session::Session, values::arena::push_list},
};

use super::{fold_plus_symbolic, nested_list_shape, terms::expand_span_3};

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

/// `Size[m]` — 嵌套列表行列形状；否则残差。
pub(crate) fn evaluate_size_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    if terms.len() != 1 {
        return Ok(push_semantic(session, SemanticOperator::Size, terms));
    }
    let term = terms[0];
    let Some((rows, cols)) = nested_list_shape(session, term) else {
        return Ok(push_semantic(session, SemanticOperator::Size, terms));
    };
    let r = session.builder().int(rows as i64, Default::default());
    let c = session.builder().int(cols as i64, Default::default());
    Ok(push_list(session, vec![r, c]))
}

/// `Sum[list]` — 向量标量和 / 矩阵按列求和。迭代器二元形式仍残差（需 Table 展开）。
pub(crate) fn evaluate_sum_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    if terms.len() != 1 {
        return Ok(push_semantic(session, SemanticOperator::Sum, terms));
    }
    let term = terms[0];
    let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(term) else {
        return Ok(push_semantic(session, SemanticOperator::Sum, vec![term]));
    };
    let items = items.clone();
    if items.is_empty() {
        return Ok(session.builder().int(0, Default::default()));
    }
    if matches!(
        session.arena.get(items[0]),
        Some(athena_ir::TermNode::Collection { elements: _, .. })
    ) {
        let Some((_, cols)) = nested_list_shape(session, term) else {
            return Ok(push_semantic(session, SemanticOperator::Sum, vec![term]));
        };
        let mut out = Vec::with_capacity(cols as usize);
        for j in 0..cols as usize {
            let mut col = Vec::with_capacity(items.len());
            for row in &items {
                let cell = match session.arena.get(*row) {
                    Some(athena_ir::TermNode::Collection { elements: cells, .. }) => cells.get(j).copied(),
                    _ => None,
                };
                let Some(cell) = cell else {
                    return Ok(push_semantic(session, SemanticOperator::Sum, vec![term]));
                };
                col.push(cell);
            }
            out.push(fold_plus_symbolic(session, col));
        }
        return Ok(push_list(session, out));
    }
    Ok(fold_plus_symbolic(session, items))
}
