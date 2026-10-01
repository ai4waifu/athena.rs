//! 结构算子（`Join` / `Range` / `Size` / `Sum` / 矩阵构造）的纯 term 折叠。

use athena_ir::SemanticOperator;
use athena_types::{Result, TermId};

use crate::{
    execution::{number_of, push_semantic},
    runtime::{session::Session, values::arena::push_list},
};

use super::{
    evaluate_arithmetic_terms, fold_plus_symbolic, fold_subtract_symbolic, nested_list_shape,
    re_eval_term, rebuild_application, symbol_name, terms::expand_span_3, try_apply_callable,
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

/// `Nearest[list, target]` — 精确整数列表上与 `target` 距离最小的元素（保留并列）。
pub(crate) fn evaluate_nearest_terms(session: &mut Session, list: TermId, target: TermId) -> Result<TermId> {
    let Some(target_n) = number_of(session, target).and_then(|v| v.as_exact_integer())
    else {
        return Ok(push_semantic(session, SemanticOperator::Nearest, vec![list, target]));
    };
    let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(list)
    else {
        return Ok(push_semantic(session, SemanticOperator::Nearest, vec![list, target]));
    };
    let items = items.clone();
    let mut pairs: Vec<(u64, TermId)> = Vec::with_capacity(items.len());
    for item in items {
        let Some(n) = number_of(session, item).and_then(|v| v.as_exact_integer())
        else {
            return Ok(push_semantic(session, SemanticOperator::Nearest, vec![list, target]));
        };
        let dist = if n >= target_n { (n - target_n) as u64 } else { (target_n - n) as u64 };
        pairs.push((dist, item));
    }
    let min_dist = pairs.iter().map(|(d, _)| *d).min().unwrap_or(0);
    let out = pairs.into_iter().filter(|(d, _)| *d == min_dist).map(|(_, id)| id).collect();
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

fn bare_even_q_head(session: &Session, pred: TermId) -> bool {
    matches!(
        session.arena.get(pred),
        Some(athena_ir::TermNode::Application {
            head: athena_ir::ApplicationHead::Semantic(SemanticOperator::EvenQ),
            arguments,
        }) if arguments.is_empty()
    )
}

/// `EvenQ[n]` — 精确整数偶性测试。
pub(crate) fn evaluate_even_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use crate::runtime::values::arena::push_bool;
    if let Some(n) = number_of(session, arg).and_then(|v| v.as_exact_integer()) {
        return Ok(push_bool(session, n % 2 == 0));
    }
    Ok(push_semantic(session, SemanticOperator::EvenQ, vec![arg]))
}

/// `IntegerQ[x]` — 精确整数或有理整数。
pub(crate) fn evaluate_integer_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use crate::runtime::values::arena::push_bool;
    if let Some(num) = number_of(session, arg) {
        let is_int = num.as_exact_integer().is_some()
            || num.as_integer().is_some()
            || num.as_rational().is_some_and(|r| r.is_integer());
        return Ok(push_bool(session, is_int));
    }
    Ok(push_semantic(session, SemanticOperator::IntegerQ, vec![arg]))
}

/// `AtomQ[expr]` — 非复合项（原子 / 符号 / 数 / Boolean）。
pub(crate) fn evaluate_atom_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use crate::runtime::values::arena::push_bool;
    let atom = matches!(session.arena.get(arg), Some(athena_ir::TermNode::Atom(_)));
    Ok(push_bool(session, atom))
}

/// `ListQ[expr]` — 有序 `Collection`（方言 List）。
pub(crate) fn evaluate_list_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use athena_types::CollectionKind;
    use crate::runtime::values::arena::push_bool;
    let list = matches!(
        session.arena.get(arg),
        Some(athena_ir::TermNode::Collection { kind: CollectionKind::OrderedCollection, .. })
    );
    Ok(push_bool(session, list))
}

/// `NumericQ[expr]` — 数字原子。
pub(crate) fn evaluate_numeric_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use crate::runtime::values::arena::push_bool;
    if number_of(session, arg).is_some() {
        return Ok(push_bool(session, true));
    }
    if matches!(session.arena.get(arg), Some(athena_ir::TermNode::Atom(_))) {
        return Ok(push_bool(session, false));
    }
    Ok(push_semantic(session, SemanticOperator::NumericQ, vec![arg]))
}

/// `NumberQ[expr]` — 数字原子（Living 16 与 `NumericQ` 同范围，保留表面名）。
pub(crate) fn evaluate_number_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    evaluate_numeric_q_terms(session, arg)
}

/// `PossibleZeroQ[expr]` — 数字零测试。
pub(crate) fn evaluate_possible_zero_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use crate::runtime::values::arena::push_bool;
    if let Some(num) = number_of(session, arg) {
        return Ok(push_bool(session, num.is_zero()));
    }
    Ok(push_semantic(session, SemanticOperator::PossibleZeroQ, vec![arg]))
}

/// `StringQ[expr]` — `Atom::String`。
pub(crate) fn evaluate_string_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use athena_ir::{Atom, TermNode};
    use crate::runtime::values::arena::push_bool;
    let string = matches!(session.arena.get(arg), Some(TermNode::Atom(Atom::String(_))));
    Ok(push_bool(session, string))
}

/// `Positive[expr]` — 数字严格大于零。
pub(crate) fn evaluate_positive_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use athena_numeric::{Number, compare as num_compare};
    use crate::runtime::values::arena::push_bool;
    use std::cmp::Ordering;
    if let Some(num) = number_of(session, arg) {
        let zero = Number::small_int(0);
        let pos = matches!(num_compare(&num, &zero), Some(Ordering::Greater));
        return Ok(push_bool(session, pos));
    }
    Ok(push_semantic(session, SemanticOperator::Positive, vec![arg]))
}

/// `VectorQ[expr]` — 平坦有序 `Collection`（含 `{}`；非嵌套行块）。
pub(crate) fn evaluate_vector_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use athena_ir::TermNode;
    use crate::runtime::values::arena::push_bool;
    let vector = match session.arena.get(arg) {
        Some(TermNode::Collection { elements: rows, .. }) => {
            rows.is_empty() || !matches!(session.arena.get(rows[0]), Some(TermNode::Collection { .. }))
        }
        _ => false,
    };
    Ok(push_bool(session, vector))
}

/// `MatrixQ[expr]` — 矩形嵌套行 `Collection`（`{{…},…}`；非空）。
pub(crate) fn evaluate_matrix_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use athena_ir::TermNode;
    use crate::runtime::values::arena::push_bool;
    let matrix = match session.arena.get(arg) {
        Some(TermNode::Collection { elements: rows, .. }) if !rows.is_empty() => {
            matches!(session.arena.get(rows[0]), Some(TermNode::Collection { .. })) && nested_list_shape(session, arg).is_some()
        }
        _ => false,
    };
    Ok(push_bool(session, matrix))
}

/// `BooleanQ[expr]` — `Atom::Boolean`。
pub(crate) fn evaluate_boolean_q_terms(session: &mut Session, arg: TermId) -> Result<TermId> {
    use athena_ir::{Atom, TermNode};
    use crate::runtime::values::arena::push_bool;
    if matches!(session.arena.get(arg), Some(TermNode::Atom(Atom::Boolean(_)))) {
        return Ok(push_bool(session, true));
    }
    Ok(push_semantic(session, SemanticOperator::BooleanQ, vec![arg]))
}

/// `MemberOf[elem, domain]` — `Element[elem, Integers]` 等集合成员测试。
pub(crate) fn evaluate_member_of_terms(session: &mut Session, elem: TermId, domain: TermId) -> Result<TermId> {
    use athena_numeric::{Number, Real};
    use crate::runtime::values::arena::push_bool;
    fn number_in_reals(num: &Number) -> bool {
        match num {
            Number::Integer(_) | Number::Rational(_) | Number::Real(_) => true,
            Number::Complex(z) => match &z.im {
                Real::Machine(x) => *x == 0.0,
                Real::Decimal(b) => b.is_zero(),
            },
            _ => false,
        }
    }
    match symbol_name(session, domain).as_deref() {
        Some("Integers") => {
            if let Some(num) = number_of(session, elem) {
                let in_integers = num.as_exact_integer().is_some()
                    || num.as_integer().is_some()
                    || num.as_rational().is_some_and(|r| r.is_integer());
                return Ok(push_bool(session, in_integers));
            }
            return Ok(push_bool(session, false));
        }
        Some("Reals") => {
            if let Some(num) = number_of(session, elem) {
                return Ok(push_bool(session, number_in_reals(&num)));
            }
            return Ok(push_bool(session, false));
        }
        _ => Ok(push_semantic(session, SemanticOperator::MemberOf, vec![elem, domain])),
    }
}

/// `Select[list, EvenQ]` — bare `EvenQ` head filters exact-integer lists.
pub(crate) fn evaluate_select_terms(session: &mut Session, list: TermId, pred: TermId) -> Result<TermId> {
    if !bare_even_q_head(session, pred) {
        return Ok(push_semantic(session, SemanticOperator::Select, vec![list, pred]));
    }
    let Some(items) = collection_elements(session, list)
    else {
        return Ok(push_semantic(session, SemanticOperator::Select, vec![list, pred]));
    };
    let mut out = Vec::new();
    for item in items {
        let Some(n) = number_of(session, item).and_then(|v| v.as_exact_integer())
        else {
            return Ok(push_semantic(session, SemanticOperator::Select, vec![list, pred]));
        };
        if n % 2 == 0 {
            out.push(item);
        }
    }
    Ok(push_list(session, out))
}

/// `ListConvolve[ker, list]` — default no-overhang convolution on exact integers.
pub(crate) fn evaluate_list_convolve_terms(session: &mut Session, ker: TermId, list: TermId) -> Result<TermId> {
    let Some(ker_items) = collection_elements(session, ker)
    else {
        return Ok(push_semantic(session, SemanticOperator::ListConvolve, vec![ker, list]));
    };
    let Some(list_items) = collection_elements(session, list)
    else {
        return Ok(push_semantic(session, SemanticOperator::ListConvolve, vec![ker, list]));
    };
    if ker_items.is_empty() || list_items.is_empty() || ker_items.len() > list_items.len() {
        return Ok(push_semantic(session, SemanticOperator::ListConvolve, vec![ker, list]));
    }
    let mut ker_vals = Vec::with_capacity(ker_items.len());
    for item in &ker_items {
        let Some(n) = number_of(session, *item).and_then(|v| v.as_exact_integer())
        else {
            return Ok(push_semantic(session, SemanticOperator::ListConvolve, vec![ker, list]));
        };
        ker_vals.push(n);
    }
    let mut list_vals = Vec::with_capacity(list_items.len());
    for item in &list_items {
        let Some(n) = number_of(session, *item).and_then(|v| v.as_exact_integer())
        else {
            return Ok(push_semantic(session, SemanticOperator::ListConvolve, vec![ker, list]));
        };
        list_vals.push(n);
    }
    let out_len = list_vals.len() - ker_vals.len() + 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let mut sum = 0i64;
        for (j, k) in ker_vals.iter().enumerate() {
            sum += k * list_vals[i + j];
        }
        out.push(session.builder().int(sum, Default::default()));
    }
    Ok(push_list(session, out))
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

/// `Sum[list]` — flat Collection fold. Nested matrix column sums are host/`MatrixRef` only.
///
/// Living 16: do not reverse-recognize nested Collections as matrices here.
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
    // Nested Collection → residual (typed matrix Sum owns the MatrixRef path).
    if matches!(session.arena.get(items[0]), Some(athena_ir::TermNode::Collection { .. })) {
        return Ok(push_semantic(session, SemanticOperator::Sum, vec![term]));
    }
    Ok(fold_plus_symbolic(session, items))
}

/// `Zeros` / `Ones` / `Eye` residual echo when the host cannot intern a typed `MatrixRef`.
///
/// Living 16: numeric constructors are owned by `ExecutionHost::apply_matrix_constructor`.
/// This helper must not rebuild nested Collection matrices.
pub(crate) fn evaluate_matrix_constructor_terms(session: &mut Session, op: SemanticOperator, terms: Vec<TermId>) -> Result<TermId> {
    Ok(push_semantic(session, op, terms))
}

/// `DiagonalMatrix` residual echo when the host cannot intern a typed `MatrixRef`.
///
/// Living 16: numeric diagonal vectors are owned by `ExecutionHost::apply_diagonal_matrix`.
/// This helper must not rebuild nested Collection matrices (including symbolic diagonals).
pub(crate) fn evaluate_diagonal_matrix_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    Ok(push_semantic(session, SemanticOperator::DiagonalMatrix, terms))
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

#[cfg(test)]
mod member_of_tests {
    use super::evaluate_member_of_terms;
    use athena_ir::{Atom, TermNode};
    use athena_numeric::{BranchPolicy, Complex, Number, Real};
    use crate::runtime::{Session, values::arena::{push_int, push_symbol_name}};

    #[test]
    fn member_of_reals_exact_integer() {
        let mut s = Session::new();
        let reals = push_symbol_name(&mut s, "Reals");
        let one = push_int(&mut s, 1);
        let out = evaluate_member_of_terms(&mut s, one, reals).expect("member");
        assert!(matches!(s.arena.get(out), Some(TermNode::Atom(Atom::Boolean(true)))));
    }

    #[test]
    fn member_of_reals_exact_rational() {
        let mut s = Session::new();
        let reals = push_symbol_name(&mut s, "Reals");
        let half = s.arena.push(
            TermNode::Atom(Atom::Number(Number::rational_i64(1, 2).expect("half"))),
            Default::default(),
        );
        let out = evaluate_member_of_terms(&mut s, half, reals).expect("member");
        assert!(matches!(s.arena.get(out), Some(TermNode::Atom(Atom::Boolean(true)))));
    }

    #[test]
    fn member_of_reals_rejects_pure_imaginary() {
        let mut s = Session::new();
        let reals = push_symbol_name(&mut s, "Reals");
        let i = s.arena.push(
            TermNode::Atom(Atom::Number(Number::complex(
                Complex::try_new(Real::machine(0.0), Real::machine(1.0), BranchPolicy::Principal).expect("i"),
            ))),
            Default::default(),
        );
        let out = evaluate_member_of_terms(&mut s, i, reals).expect("member");
        assert!(matches!(s.arena.get(out), Some(TermNode::Atom(Atom::Boolean(false)))));
    }
}

#[cfg(test)]
mod nearest_tests {
    use super::evaluate_nearest_terms;
    use crate::runtime::{Session, values::arena::{push_int, push_list}};

    #[test]
    fn nearest_exact_integer_ties() {
        let mut s = Session::new();
        let i1 = push_int(&mut s, 1);
        let i2 = push_int(&mut s, 2);
        let i4 = push_int(&mut s, 4);
        let list = push_list(&mut s, vec![i1, i2, i4]);
        let target = push_int(&mut s, 3);
        let out = evaluate_nearest_terms(&mut s, list, target).expect("nearest");
        let items = match s.arena.get(out) {
            Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
            _ => panic!("expected list"),
        };
        assert_eq!(items.len(), 2);
        assert!(s.arena.structural_eq(items[0], i2));
        assert!(s.arena.structural_eq(items[1], i4));
    }
}
