//! 结构算子（`Join` / `Range` / `Size` / `Sum` / `Determinant` / 矩阵构造）的纯 term 折叠。

use athena_ir::SemanticOperator;
use athena_types::{Diagnostic, Result, TermId};

use crate::{
    domains::linear_algebra::det_bareiss,
    execution::{number_of, push_semantic},
    runtime::{session::Session, values::arena::push_list},
};

use super::{
    evaluate_arithmetic_terms, fold_plus_symbolic, nested_list_shape, parse_matrix_dims, rational_to_term_session,
    term_to_rational_matrix_session, terms::expand_span_3,
};

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
    let ints = terms.iter().map(|t| number_of(session, *t).and_then(|n| n.as_exact_integer())).collect::<Option<Vec<_>>>();
    let Some(ints) = ints
    else {
        return Ok(push_semantic(session, SemanticOperator::Range, terms));
    };
    let bounds = match ints.as_slice() {
        [n] => Some((1, *n, 1)),
        [a, b] => Some((*a, *b, 1)),
        [a, b, step] => Some((*a, *b, *step)),
        _ => None,
    };
    let Some((a, b, step)) = bounds
    else {
        return Ok(push_semantic(session, SemanticOperator::Range, terms));
    };
    let Some(values) = expand_span_3(a, step, b)
    else {
        return Ok(push_semantic(session, SemanticOperator::Range, terms));
    };
    let out: Vec<TermId> = values.into_iter().map(|v| session.builder().int(v, Default::default())).collect();
    Ok(push_list(session, out))
}

/// `Size[m]` — 嵌套列表行列形状；否则残差。
pub(crate) fn evaluate_size_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    if terms.len() != 1 {
        return Ok(push_semantic(session, SemanticOperator::Size, terms));
    }
    let term = terms[0];
    let Some((rows, cols)) = nested_list_shape(session, term)
    else {
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
    let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(term)
    else {
        return Ok(push_semantic(session, SemanticOperator::Sum, vec![term]));
    };
    let items = items.clone();
    if items.is_empty() {
        return Ok(session.builder().int(0, Default::default()));
    }
    if matches!(session.arena.get(items[0]), Some(athena_ir::TermNode::Collection { elements: _, .. })) {
        let Some((_, cols)) = nested_list_shape(session, term)
        else {
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
                let Some(cell) = cell
                else {
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

/// `Determinant[m]` — 有理矩阵 Bareiss；非矩阵或失败时残差，失败诊断可选。
pub(crate) fn evaluate_determinant_term(session: &mut Session, term: TermId) -> Result<(TermId, Option<Diagnostic>)> {
    let echo = push_semantic(session, SemanticOperator::Determinant, vec![term]);
    let Some(matrix) = term_to_rational_matrix_session(session, term)
    else {
        return Ok((echo, None));
    };
    match det_bareiss(&matrix) {
        Ok(result) => Ok((rational_to_term_session(session, &result.det), None)),
        Err(diagnostic) => Ok((echo, Some(diagnostic))),
    }
}

/// `Zeros` / `Ones` / `Eye` — 按维度构造有理整数矩阵；非法维度则残差。
pub(crate) fn evaluate_matrix_constructor_terms(session: &mut Session, op: SemanticOperator, terms: Vec<TermId>) -> Result<TermId> {
    let Some((rows, cols)) = parse_matrix_dims(session, &terms)
    else {
        return Ok(push_semantic(session, op, terms));
    };
    let n = match rows.checked_mul(cols) {
        Some(v) if v <= 4096 => v as usize,
        _ => return Ok(push_semantic(session, op, terms)),
    };
    if n == 0 {
        return Ok(push_list(session, Vec::new()));
    }
    let fill = match op {
        SemanticOperator::Ones => 1i64,
        SemanticOperator::Zeros | SemanticOperator::Eye => 0,
        _ => return Ok(push_semantic(session, op, terms)),
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
    Ok(push_list(session, rows_out))
}

/// `ElementwiseMultiply` / `ElementwiseDivide` / `ElementwisePower` — 集合 zip + 标量广播。
pub(crate) fn evaluate_elementwise_terms(session: &mut Session, op: SemanticOperator, left: TermId, right: TermId) -> Result<TermId> {
    let echo = push_semantic(session, op, vec![left, right]);
    let scalar_op = match op {
        SemanticOperator::ElementwiseMultiply => SemanticOperator::Multiply,
        SemanticOperator::ElementwiseDivide => SemanticOperator::Divide,
        SemanticOperator::ElementwisePower => SemanticOperator::Power,
        _ => return Ok(echo),
    };
    match elementwise_zip(session, scalar_op, left, right)? {
        Some(term) => Ok(term),
        None => Ok(echo),
    }
}

fn elementwise_zip(session: &mut Session, scalar_op: SemanticOperator, left: TermId, right: TermId) -> Result<Option<TermId>> {
    let left_is_collection = matches!(session.arena.get(left), Some(athena_ir::TermNode::Collection { .. }));
    let right_is_collection = matches!(session.arena.get(right), Some(athena_ir::TermNode::Collection { .. }));
    match (left_is_collection, right_is_collection) {
        (true, true) => {
            let a = match session.arena.get(left) {
                Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
                _ => return Ok(None),
            };
            let b = match session.arena.get(right) {
                Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
                _ => return Ok(None),
            };
            if a.len() != b.len() {
                return Ok(None);
            }
            let mut out = Vec::with_capacity(a.len());
            for (lhs, rhs) in a.into_iter().zip(b.into_iter()) {
                match elementwise_zip(session, scalar_op, lhs, rhs)? {
                    Some(term) => out.push(term),
                    None => return Ok(None),
                }
            }
            Ok(Some(push_list(session, out)))
        }
        (true, false) => {
            let a = match session.arena.get(left) {
                Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
                _ => return Ok(None),
            };
            let mut out = Vec::with_capacity(a.len());
            for lhs in a {
                match elementwise_zip(session, scalar_op, lhs, right)? {
                    Some(term) => out.push(term),
                    None => return Ok(None),
                }
            }
            Ok(Some(push_list(session, out)))
        }
        (false, true) => {
            let b = match session.arena.get(right) {
                Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
                _ => return Ok(None),
            };
            let mut out = Vec::with_capacity(b.len());
            for rhs in b {
                match elementwise_zip(session, scalar_op, left, rhs)? {
                    Some(term) => out.push(term),
                    None => return Ok(None),
                }
            }
            Ok(Some(push_list(session, out)))
        }
        (false, false) => Ok(Some(evaluate_arithmetic_terms(session, scalar_op, vec![left, right])?)),
    }
}
