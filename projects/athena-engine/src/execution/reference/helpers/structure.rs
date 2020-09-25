//! 结构算子（`Join` / `Range` / `Size` / `Sum` / `Determinant` / 矩阵构造）的纯 term 折叠。

use athena_ir::SemanticOperator;
use athena_types::{Diagnostic, Result, TermId};

use crate::{
    domains::linear_algebra::det_bareiss,
    execution::{number_of, push_semantic},
    runtime::{session::Session, values::arena::push_list},
};

use super::{
    evaluate_arithmetic_terms, fold_plus_symbolic, fold_subtract_symbolic, nested_list_shape, parse_matrix_dims,
    rational_to_term_session, re_eval_term, rebuild_application, term_to_rational_matrix_session, terms::expand_span_3,
    try_apply_callable,
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

/// `Take[list, n]` — 取前 `n` 个元素（`n ≥ 0`）；否则残差。
pub(crate) fn evaluate_take_terms(session: &mut Session, list: TermId, count: TermId) -> Result<TermId> {
    let Some(n) = number_of(session, count).and_then(|v| v.as_exact_integer())
    else {
        return Ok(push_semantic(session, SemanticOperator::Take, vec![list, count]));
    };
    if n < 0 {
        return Ok(push_semantic(session, SemanticOperator::Take, vec![list, count]));
    }
    let n = n as usize;
    match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => {
            let end = n.min(items.len());
            let taken = items[..end].to_vec();
            Ok(push_list(session, taken))
        }
        Some(athena_ir::TermNode::Application { head, arguments }) => {
            let head = *head;
            let end = n.min(arguments.len());
            let taken = arguments[..end].to_vec();
            Ok(session.builder().application(head, taken, Default::default()))
        }
        _ => Ok(push_semantic(session, SemanticOperator::Take, vec![list, count])),
    }
}

/// `Drop[list, n]` — 丢弃前 `n` 个元素（`n ≥ 0`）；否则残差。
pub(crate) fn evaluate_drop_terms(session: &mut Session, list: TermId, count: TermId) -> Result<TermId> {
    let Some(n) = number_of(session, count).and_then(|v| v.as_exact_integer())
    else {
        return Ok(push_semantic(session, SemanticOperator::Drop, vec![list, count]));
    };
    if n < 0 {
        return Ok(push_semantic(session, SemanticOperator::Drop, vec![list, count]));
    }
    let n = n as usize;
    match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => {
            let start = n.min(items.len());
            let dropped = items[start..].to_vec();
            Ok(push_list(session, dropped))
        }
        Some(athena_ir::TermNode::Application { head, arguments }) => {
            let head = *head;
            let start = n.min(arguments.len());
            let dropped = arguments[start..].to_vec();
            Ok(session.builder().application(head, dropped, Default::default()))
        }
        _ => Ok(push_semantic(session, SemanticOperator::Drop, vec![list, count])),
    }
}

/// `Append[list, elem]` — 末尾追加；非集合则残差。
pub(crate) fn evaluate_append_terms(session: &mut Session, list: TermId, elem: TermId) -> Result<TermId> {
    match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => {
            let mut out = items.clone();
            out.push(elem);
            Ok(push_list(session, out))
        }
        Some(athena_ir::TermNode::Application { head, arguments }) => {
            let head = *head;
            let mut out = arguments.clone();
            out.push(elem);
            Ok(session.builder().application(head, out, Default::default()))
        }
        _ => Ok(push_semantic(session, SemanticOperator::Append, vec![list, elem])),
    }
}

/// `Prepend[list, elem]` — 头部插入；非集合则残差。
pub(crate) fn evaluate_prepend_terms(session: &mut Session, list: TermId, elem: TermId) -> Result<TermId> {
    match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => {
            let mut out = Vec::with_capacity(items.len() + 1);
            out.push(elem);
            out.extend_from_slice(items);
            Ok(push_list(session, out))
        }
        Some(athena_ir::TermNode::Application { head, arguments }) => {
            let head = *head;
            let mut out = Vec::with_capacity(arguments.len() + 1);
            out.push(elem);
            out.extend_from_slice(arguments);
            Ok(session.builder().application(head, out, Default::default()))
        }
        _ => Ok(push_semantic(session, SemanticOperator::Prepend, vec![list, elem])),
    }
}

/// `MemberQ[list, elem]` — 结构相等成员测试。
pub(crate) fn evaluate_member_q_terms(session: &mut Session, list: TermId, elem: TermId) -> Result<TermId> {
    use crate::runtime::values::arena::push_bool;
    match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => {
            let found = items.iter().any(|item| session.arena.structural_eq(*item, elem));
            Ok(push_bool(session, found))
        }
        Some(athena_ir::TermNode::Application { arguments, .. }) => {
            let found = arguments.iter().any(|item| session.arena.structural_eq(*item, elem));
            Ok(push_bool(session, found))
        }
        _ => Ok(push_semantic(session, SemanticOperator::MemberQ, vec![list, elem])),
    }
}

/// `Sort[list]` — 全部为精确整数时按升序排序，否则残差。
pub(crate) fn evaluate_sort_terms(session: &mut Session, list: TermId) -> Result<TermId> {
    let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(list)
    else {
        return Ok(push_semantic(session, SemanticOperator::Sort, vec![list]));
    };
    let items = items.clone();
    let mut pairs: Vec<(i64, TermId)> = Vec::with_capacity(items.len());
    for item in items {
        let Some(n) = number_of(session, item).and_then(|v| v.as_exact_integer())
        else {
            return Ok(push_semantic(session, SemanticOperator::Sort, vec![list]));
        };
        pairs.push((n, item));
    }
    pairs.sort_by_key(|(n, _)| *n);
    Ok(push_list(session, pairs.into_iter().map(|(_, id)| id).collect()))
}

/// `DeleteDuplicates[list]` — 按结构相等保留首次出现。
pub(crate) fn evaluate_delete_duplicates_terms(session: &mut Session, list: TermId) -> Result<TermId> {
    let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(list)
    else {
        return Ok(push_semantic(session, SemanticOperator::DeleteDuplicates, vec![list]));
    };
    let items = items.clone();
    let mut out = Vec::new();
    for item in items {
        if !out.iter().any(|seen| session.arena.structural_eq(*seen, item)) {
            out.push(item);
        }
    }
    Ok(push_list(session, out))
}

/// `Count[list, elem]` — 结构相等出现次数。
pub(crate) fn evaluate_count_terms(session: &mut Session, list: TermId, elem: TermId) -> Result<TermId> {
    match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => {
            let n = items.iter().filter(|item| session.arena.structural_eq(**item, elem)).count() as i64;
            Ok(session.builder().int(n, Default::default()))
        }
        Some(athena_ir::TermNode::Application { arguments, .. }) => {
            let n = arguments.iter().filter(|item| session.arena.structural_eq(**item, elem)).count() as i64;
            Ok(session.builder().int(n, Default::default()))
        }
        _ => Ok(push_semantic(session, SemanticOperator::Count, vec![list, elem])),
    }
}

/// `Partition[list, n]` — 按长度 `n` 切块（丢弃不足一块的尾部）。
pub(crate) fn evaluate_partition_terms(session: &mut Session, list: TermId, size: TermId) -> Result<TermId> {
    let Some(n) = number_of(session, size).and_then(|v| v.as_exact_integer())
    else {
        return Ok(push_semantic(session, SemanticOperator::Partition, vec![list, size]));
    };
    if n <= 0 {
        return Ok(push_semantic(session, SemanticOperator::Partition, vec![list, size]));
    }
    let n = n as usize;
    let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(list)
    else {
        return Ok(push_semantic(session, SemanticOperator::Partition, vec![list, size]));
    };
    let items = items.clone();
    let mut out = Vec::new();
    let mut i = 0;
    while i + n <= items.len() {
        let chunk = push_list(session, items[i..i + n].to_vec());
        out.push(chunk);
        i += n;
    }
    Ok(push_list(session, out))
}

/// `ConstantArray[elem, n]` — 长度 `n` 的常数列表。
pub(crate) fn evaluate_constant_array_terms(session: &mut Session, elem: TermId, count: TermId) -> Result<TermId> {
    let Some(n) = number_of(session, count).and_then(|v| v.as_exact_integer())
    else {
        return Ok(push_semantic(session, SemanticOperator::ConstantArray, vec![elem, count]));
    };
    if n < 0 {
        return Ok(push_semantic(session, SemanticOperator::ConstantArray, vec![elem, count]));
    }
    Ok(push_list(session, vec![elem; n as usize]))
}

fn collection_elements(session: &Session, list: TermId) -> Option<Vec<TermId>> {
    match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => Some(items.clone()),
        _ => None,
    }
}

fn sort_exact_integer_ids(session: &Session, items: &[TermId]) -> Option<Vec<TermId>> {
    let mut pairs: Vec<(i64, TermId)> = Vec::with_capacity(items.len());
    for item in items {
        let n = number_of(session, *item)?.as_exact_integer()?;
        pairs.push((n, *item));
    }
    pairs.sort_by_key(|(n, _)| *n);
    Some(pairs.into_iter().map(|(_, id)| id).collect())
}

/// `Union[list…]` — 展平并结构去重；全为精确整数时升序。
pub(crate) fn evaluate_union_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    let mut merged = Vec::new();
    for term in &terms {
        let Some(items) = collection_elements(session, *term)
        else {
            return Ok(push_semantic(session, SemanticOperator::Union, terms));
        };
        for item in items {
            if !merged.iter().any(|seen| session.arena.structural_eq(*seen, item)) {
                merged.push(item);
            }
        }
    }
    if let Some(sorted) = sort_exact_integer_ids(session, &merged) {
        return Ok(push_list(session, sorted));
    }
    Ok(push_list(session, merged))
}

/// `Intersection[list…]` — 出现在所有列表中的元素；全精确整数时升序。
pub(crate) fn evaluate_intersection_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    if terms.is_empty() {
        return Ok(push_list(session, Vec::new()));
    }
    let mut lists = Vec::with_capacity(terms.len());
    for term in &terms {
        let Some(items) = collection_elements(session, *term)
        else {
            return Ok(push_semantic(session, SemanticOperator::Intersection, terms));
        };
        lists.push(items);
    }
    let first = &lists[0];
    let mut out = Vec::new();
    for item in first {
        if out.iter().any(|seen| session.arena.structural_eq(*seen, *item)) {
            continue;
        }
        let in_all = lists[1..].iter().all(|list| list.iter().any(|x| session.arena.structural_eq(*x, *item)));
        if in_all {
            out.push(*item);
        }
    }
    if let Some(sorted) = sort_exact_integer_ids(session, &out) {
        return Ok(push_list(session, sorted));
    }
    Ok(push_list(session, out))
}

/// `Accumulate[list]` — 前缀和。
pub(crate) fn evaluate_accumulate_terms(session: &mut Session, list: TermId) -> Result<TermId> {
    let Some(items) = collection_elements(session, list)
    else {
        return Ok(push_semantic(session, SemanticOperator::Accumulate, vec![list]));
    };
    if items.is_empty() {
        return Ok(push_list(session, Vec::new()));
    }
    let mut out = Vec::with_capacity(items.len());
    let mut running = items[0];
    out.push(running);
    for item in items.into_iter().skip(1) {
        running = fold_plus_symbolic(session, vec![running, item]);
        out.push(running);
    }
    Ok(push_list(session, out))
}

/// `Differences[list]` — 相邻差分 `aᵢ₊₁ - aᵢ`。
pub(crate) fn evaluate_differences_terms(session: &mut Session, list: TermId) -> Result<TermId> {
    let Some(items) = collection_elements(session, list)
    else {
        return Ok(push_semantic(session, SemanticOperator::Differences, vec![list]));
    };
    if items.len() < 2 {
        return Ok(push_list(session, Vec::new()));
    }
    let mut out = Vec::with_capacity(items.len() - 1);
    for window in items.windows(2) {
        let diff = fold_subtract_symbolic(session, vec![window[1], window[0]]);
        out.push(diff);
    }
    Ok(push_list(session, out))
}

/// `FreeQ[list, elem]` — 顶层无结构相等成员。
pub(crate) fn evaluate_free_q_terms(session: &mut Session, list: TermId, elem: TermId) -> Result<TermId> {
    use crate::runtime::values::arena::push_bool;
    match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => {
            let free = !items.iter().any(|item| session.arena.structural_eq(*item, elem));
            Ok(push_bool(session, free))
        }
        Some(athena_ir::TermNode::Application { arguments, .. }) => {
            let free = !arguments.iter().any(|item| session.arena.structural_eq(*item, elem));
            Ok(push_bool(session, free))
        }
        _ => Ok(push_semantic(session, SemanticOperator::FreeQ, vec![list, elem])),
    }
}

/// `Extract[list, n]` — 1-based 整数下标提取。
pub(crate) fn evaluate_extract_terms(session: &mut Session, list: TermId, index: TermId) -> Result<TermId> {
    let Some(n) = number_of(session, index).and_then(|v| v.as_exact_integer())
    else {
        return Ok(push_semantic(session, SemanticOperator::Extract, vec![list, index]));
    };
    if n <= 0 {
        return Ok(push_semantic(session, SemanticOperator::Extract, vec![list, index]));
    }
    let idx = (n as usize) - 1;
    match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) if idx < items.len() => Ok(items[idx]),
        Some(athena_ir::TermNode::Application { arguments, .. }) if idx < arguments.len() => Ok(arguments[idx]),
        _ => Ok(push_semantic(session, SemanticOperator::Extract, vec![list, index])),
    }
}

/// `PadLeft[list, n]` — 左侧用 `0` 填充到长度 `n`（已更长则截断左侧）。
pub(crate) fn evaluate_pad_left_terms(session: &mut Session, list: TermId, len: TermId) -> Result<TermId> {
    let Some(n) = number_of(session, len).and_then(|v| v.as_exact_integer())
    else {
        return Ok(push_semantic(session, SemanticOperator::PadLeft, vec![list, len]));
    };
    if n < 0 {
        return Ok(push_semantic(session, SemanticOperator::PadLeft, vec![list, len]));
    }
    let n = n as usize;
    let Some(items) = collection_elements(session, list)
    else {
        return Ok(push_semantic(session, SemanticOperator::PadLeft, vec![list, len]));
    };
    let zero = session.builder().int(0, Default::default());
    let mut out = Vec::with_capacity(n);
    if items.len() >= n {
        out.extend_from_slice(&items[items.len() - n..]);
    }
    else {
        for _ in 0..(n - items.len()) {
            out.push(zero);
        }
        out.extend_from_slice(&items);
    }
    Ok(push_list(session, out))
}

/// `Riffle[a, b]` — 交错两列表元素；长度取较短一侧。
pub(crate) fn evaluate_riffle_terms(session: &mut Session, left: TermId, right: TermId) -> Result<TermId> {
    let Some(a) = collection_elements(session, left)
    else {
        return Ok(push_semantic(session, SemanticOperator::Riffle, vec![left, right]));
    };
    let Some(b) = collection_elements(session, right)
    else {
        return Ok(push_semantic(session, SemanticOperator::Riffle, vec![left, right]));
    };
    let n = a.len().min(b.len());
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        out.push(a[i]);
        out.push(b[i]);
    }
    Ok(push_list(session, out))
}

/// `Position[list, elem]` — 顶层 1-based 位置列表 `{{i},…}`。
pub(crate) fn evaluate_position_terms(session: &mut Session, list: TermId, elem: TermId) -> Result<TermId> {
    let Some(items) = collection_elements(session, list)
    else {
        return Ok(push_semantic(session, SemanticOperator::Position, vec![list, elem]));
    };
    let mut out = Vec::new();
    for (i, item) in items.into_iter().enumerate() {
        if session.arena.structural_eq(item, elem) {
            let idx = session.builder().int((i as i64) + 1, Default::default());
            out.push(push_list(session, vec![idx]));
        }
    }
    Ok(push_list(session, out))
}

/// `Array[f, n]` — `{f[1],…,f[n]}`。
pub(crate) fn evaluate_array_terms(session: &mut Session, func: TermId, count: TermId) -> Result<TermId> {
    let Some(n) = number_of(session, count).and_then(|v| v.as_exact_integer())
    else {
        return Ok(push_semantic(session, SemanticOperator::Array, vec![func, count]));
    };
    if n < 0 {
        return Ok(push_semantic(session, SemanticOperator::Array, vec![func, count]));
    }
    let mut out = Vec::with_capacity(n as usize);
    for i in 1..=n {
        let idx = session.builder().int(i, Default::default());
        if let Some(term) = try_apply_callable(session, func, &[idx])? {
            out.push(term);
            continue;
        }
        let app = rebuild_application(session, func, vec![idx]);
        out.push(re_eval_term(session, app)?);
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

/// `DiagonalMatrix[{d0,…}]` — 对角元向量构造成方阵；非法则残差。
pub(crate) fn evaluate_diagonal_matrix_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    let echo = push_semantic(session, SemanticOperator::DiagonalMatrix, terms.clone());
    if terms.len() != 1 {
        return Ok(echo);
    }
    let Some(athena_ir::TermNode::Collection { elements: diag, .. }) = session.arena.get(terms[0])
    else {
        return Ok(echo);
    };
    let diag = diag.clone();
    let n = diag.len();
    if n == 0 {
        return Ok(push_list(session, Vec::new()));
    }
    if n > 4096 {
        return Ok(echo);
    }
    let mut rows_out = Vec::with_capacity(n);
    for r in 0..n {
        let mut row = Vec::with_capacity(n);
        for c in 0..n {
            if r == c {
                row.push(diag[r]);
            }
            else {
                row.push(session.builder().int(0, Default::default()));
            }
        }
        rows_out.push(push_list(session, row));
    }
    Ok(push_list(session, rows_out))
}

/// `ElementwiseMultiply` / `ElementwiseDivide` / `ElementwisePower` /
/// `ElementwiseAnd` / `ElementwiseOr` — 集合 zip + 标量广播。
pub(crate) fn evaluate_elementwise_terms(session: &mut Session, op: SemanticOperator, left: TermId, right: TermId) -> Result<TermId> {
    let echo = push_semantic(session, op, vec![left, right]);
    match op {
        SemanticOperator::ElementwiseAnd | SemanticOperator::ElementwiseOr => {
            match elementwise_logical_zip(session, op, left, right)? {
                Some(term) => Ok(term),
                None => Ok(echo),
            }
        }
        SemanticOperator::ElementwiseMultiply | SemanticOperator::ElementwiseDivide | SemanticOperator::ElementwisePower => {
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
        _ => Ok(echo),
    }
}

fn elementwise_logical_zip(session: &mut Session, op: SemanticOperator, left: TermId, right: TermId) -> Result<Option<TermId>> {
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
                match elementwise_logical_zip(session, op, lhs, rhs)? {
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
                match elementwise_logical_zip(session, op, lhs, right)? {
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
                match elementwise_logical_zip(session, op, left, rhs)? {
                    Some(term) => out.push(term),
                    None => return Ok(None),
                }
            }
            Ok(Some(push_list(session, out)))
        }
        (false, false) => {
            let Some(a) = super::as_boolean_like_term(session, left)
            else {
                return Ok(None);
            };
            let Some(b) = super::as_boolean_like_term(session, right)
            else {
                return Ok(None);
            };
            let value = match op {
                SemanticOperator::ElementwiseAnd => a && b,
                SemanticOperator::ElementwiseOr => a || b,
                _ => return Ok(None),
            };
            Ok(Some(session.builder().boolean(value, Default::default())))
        }
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
