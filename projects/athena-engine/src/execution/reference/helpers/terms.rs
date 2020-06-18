//! Reference 执行器的项 / 矩阵 / 迭代器辅助。

use athena_numeric::{Integer, Number, Rational, to_f64_lossy as num_to_f64_lossy};
use athena_types::{Result, SymbolId, TermId};

use super::diag;
use athena_ir::{ApplicationHead, Atom, MathematicalConstant, SemanticOperator, TermBuilder, UnaryFunction};

use crate::{
    api::request::AthenaRequest,
    domains::linear_algebra::{MatrixEntry, MatrixValue},
    execution::{execute_ir_request, number_of, push_number, push_semantic},
    runtime::{
        session::Session,
        values::{
            arena::push_list,
            numeric_clone::{clone_integer, clone_number, clone_rational},
        },
    },
};

/// 编译并再求值一项（失败则保留原项）。共享给 `Map` / `Apply` / iterator fold。
pub(crate) fn re_eval_term(session: &mut Session, term: TermId) -> Result<TermId> {
    match execute_ir_request(session, AthenaRequest::Term(term)) {
        Ok(result_id) => Ok(session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(term)),
        Err(_) => Ok(term),
    }
}

fn is_sem(head: ApplicationHead, op: SemanticOperator) -> bool {
    matches!(head, ApplicationHead::Semantic(o) if o == op)
}

fn is_math_constant(session: &Session, id: TermId, expected: MathematicalConstant) -> bool {
    matches!(
        session.arena.get(id),
        Some(athena_ir::TermNode::Atom(Atom::Constant(c))) if *c == expected
    )
}

fn head_label(session: &Session, head: ApplicationHead) -> Option<String> {
    match head {
        ApplicationHead::Semantic(op) => Some(op.debug_label().to_string()),
        ApplicationHead::Extension(id) => session.extensions.display_name(id).map(str::to_string),
    }
}

pub(crate) fn eval_trig_exact_session(session: &mut Session, function: UnaryFunction, arg: TermId) -> Option<TermId> {
    let angle = normalize_pi_angle_session(session, arg)?;
    match function {
        UnaryFunction::Sin => Some(session.builder().int(0, Default::default())),
        UnaryFunction::Cos => Some(session.builder().int(if angle % 2 == 0 { 1 } else { -1 }, Default::default())),
        UnaryFunction::Tan if angle % 2 == 0 => Some(session.builder().int(0, Default::default())),
        _ => None,
    }
}

pub(crate) fn term_as_f64_session(session: &Session, arg: TermId) -> Option<f64> {
    if let Some(k) = normalize_pi_angle_session(session, arg) {
        return Some((k as f64) * std::f64::consts::PI);
    }
    if is_math_constant(session, arg, MathematicalConstant::EulerNumber) {
        return Some(std::f64::consts::E);
    }
    number_of(session, arg).and_then(num_to_f64_lossy)
}

pub(crate) fn normalize_pi_angle_session(session: &Session, arg: TermId) -> Option<i64> {
    if number_of(session, arg).is_some_and(|n| n.is_zero()) {
        return Some(0);
    }
    if is_math_constant(session, arg, MathematicalConstant::Pi) {
        return Some(1);
    }
    if let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(arg) {
        if is_sem(*head, SemanticOperator::Multiply) {
            if let [a, b] = arguments.as_slice() {
                if is_math_constant(session, *a, MathematicalConstant::Pi) {
                    return number_of(session, *b).and_then(|n| n.as_exact_integer());
                }
                if is_math_constant(session, *b, MathematicalConstant::Pi) {
                    return number_of(session, *a).and_then(|n| n.as_exact_integer());
                }
            }
        }
        if is_sem(*head, SemanticOperator::Add) && arguments.len() == 1 && is_math_constant(session, arguments[0], MathematicalConstant::Pi) {
            return Some(1);
        }
    }
    None
}

/// 仅用于调试 / 诊断的头标签 — **不得**用于语义分派。
pub(crate) fn debug_head_label_session(session: &Session, id: TermId) -> Option<String> {
    match session.arena.get(id)? {
        athena_ir::TermNode::Application { head, .. } => head_label(session, *head),
        athena_ir::TermNode::Atom(Atom::Symbol(symbol)) => session.arena.symbols().resolve(*symbol).map(str::to_string),
        athena_ir::TermNode::Atom(Atom::Constant(c)) => Some(c.debug_label().to_string()),
        _ => None,
    }
}

pub(crate) fn expand_span_2(a: i64, b: i64) -> Option<Vec<i64>> {
    let mut out = Vec::new();
    if a <= b {
        let mut x = a;
        while x <= b {
            out.push(x);
            x += 1;
        }
    }
    else {
        let mut x = a;
        while x >= b {
            out.push(x);
            x -= 1;
        }
    }
    Some(out)
}

pub(crate) fn expand_span_3(a: i64, step: i64, b: i64) -> Option<Vec<i64>> {
    if step == 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut x = a;
    if step > 0 {
        while x <= b {
            out.push(x);
            x += step;
        }
    }
    else {
        while x >= b {
            out.push(x);
            x += step;
        }
    }
    Some(out)
}

/// 为迭代器 `Sum` 展开 `{i,n}` / `{i,a,b}` / `{i,a,b,step}` / `{n}`。
pub(crate) fn expand_iterator_session(session: &mut Session, spec: TermId) -> Option<(Option<SymbolId>, Vec<TermId>)> {
    let items = match session.arena.get(spec) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
        _ => return None,
    };
    match items.as_slice() {
        [var, n] => {
            let sym = term_symbol_id(session, *var)?;
            let n = number_of(session, *n)?.as_exact_integer()?;
            Some((Some(sym), range_int_terms(session, 1, n, 1)?))
        }
        [var, a, b] => {
            let sym = term_symbol_id(session, *var)?;
            let a = number_of(session, *a)?.as_exact_integer()?;
            let b = number_of(session, *b)?.as_exact_integer()?;
            Some((Some(sym), range_int_terms(session, a, b, 1)?))
        }
        [var, a, b, step] => {
            let sym = term_symbol_id(session, *var)?;
            let a = number_of(session, *a)?.as_exact_integer()?;
            let b = number_of(session, *b)?.as_exact_integer()?;
            let step = number_of(session, *step)?.as_exact_integer()?;
            Some((Some(sym), range_int_terms(session, a, b, step)?))
        }
        [n] => {
            let n = number_of(session, *n)?.as_exact_integer()?;
            Some((None, range_int_terms(session, 1, n, 1)?))
        }
        _ => None,
    }
}

pub(crate) fn term_symbol_id(session: &Session, id: TermId) -> Option<SymbolId> {
    match session.arena.get(id) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(s))) => Some(*s),
        _ => None,
    }
}

pub(crate) fn range_int_terms(session: &mut Session, a: i64, b: i64, step: i64) -> Option<Vec<TermId>> {
    let ints = expand_span_3(a, step, b)?;
    Some(ints.into_iter().map(|n| session.builder().int(n, Default::default())).collect())
}

pub(crate) fn rebuild_application(session: &mut Session, head: TermId, args: Vec<TermId>) -> TermId {
    match session.arena.get(head) {
        // 零元语义 / 扩展应用用作算子值（不是 Symbol 显示名）。
        Some(athena_ir::TermNode::Application { head: ApplicationHead::Semantic(op), arguments }) if arguments.is_empty() => {
            push_semantic(session, *op, args)
        }
        Some(athena_ir::TermNode::Application { head: ApplicationHead::Extension(id), arguments }) if arguments.is_empty() => {
            let id = *id;
            let mut b = TermBuilder::new(&mut session.arena);
            b.application_extension_id(id, args, athena_ir::TermNode::default_span())
        }
        // 禁止裸 `Symbol` 经显示名 `extensions.intern`；保留 typed `ApplyHead` 残差。
        _ => {
            let mut wrapped = Vec::with_capacity(args.len() + 1);
            wrapped.push(head);
            wrapped.extend(args);
            push_semantic(session, SemanticOperator::ApplyHead, wrapped)
        }
    }
}

pub(crate) fn symbol_name(session: &Session, id: TermId) -> Option<String> {
    match session.arena.get(id) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol))) => session.arena.symbols().resolve(*symbol).map(str::to_string),
        _ => None,
    }
}

pub(crate) fn parse_matrix_dims(session: &Session, args: &[TermId]) -> Option<(u64, u64)> {
    let as_dim = |t: TermId| -> Option<u64> {
        let n = number_of(session, t)?.as_exact_integer()?;
        if n < 0 { None } else { Some(n as u64) }
    };
    match args {
        [n] => {
            let n = as_dim(*n)?;
            Some((n, n))
        }
        [m, n] => Some((as_dim(*m)?, as_dim(*n)?)),
        _ => None,
    }
}

pub(crate) fn collect_rule_pairs(session: &Session, rules_term: TermId) -> Vec<(TermId, TermId)> {
    match session.arena.get(rules_term) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.iter().filter_map(|r| rule_pair(session, *r)).collect(),
        _ => rule_pair(session, rules_term).into_iter().collect(),
    }
}

pub(crate) fn rule_pair(session: &Session, expr: TermId) -> Option<(TermId, TermId)> {
    let athena_ir::TermNode::Application { head, arguments } = session.arena.get(expr)?
    else {
        return None;
    };
    if arguments.len() != 2 {
        return None;
    }
    match *head {
        ApplicationHead::Semantic(SemanticOperator::Rule | SemanticOperator::RuleDeferred) => Some((arguments[0], arguments[1])),
        _ => None,
    }
}

/// 构造 `Rule` / `RuleDeferred` 残差应用（不求值左右部）。
pub(crate) fn evaluate_rule_terms(
    session: &mut Session,
    op: SemanticOperator,
    lhs: TermId,
    rhs: TermId,
) -> Result<TermId> {
    if !matches!(op, SemanticOperator::Rule | SemanticOperator::RuleDeferred) {
        return Err(diag("rule_operator_expected"));
    }
    Ok(push_semantic(session, op, vec![lhs, rhs]))
}

/// `Matches[expr, pat]` → Boolean。
pub(crate) fn evaluate_matches_terms(session: &mut Session, expr: TermId, pat: TermId) -> Result<bool> {
    Ok(crate::execution::builtins::patterns::pattern_matches(session, expr, pat))
}

/// `CollectMatches[list, pat]` — 按 pattern 过滤列表元素。
pub(crate) fn evaluate_collect_matches_terms(session: &mut Session, list: TermId, pat: TermId) -> Result<TermId> {
    let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(list) else {
        return Ok(push_semantic(session, SemanticOperator::CollectMatches, vec![list, pat]));
    };
    let items = items.clone();
    let mut out = Vec::new();
    for item in items {
        if crate::execution::builtins::patterns::pattern_matches(session, item, pat) {
            out.push(item);
        }
    }
    Ok(push_list(session, out))
}

/// `ReplaceAll[expr, rules]` — 字面替换后再走 `execute_ir_request` 求值。
pub(crate) fn evaluate_replace_all_terms(session: &mut Session, expr: TermId, rules_term: TermId) -> Result<TermId> {
    use crate::api::request::AthenaRequest;
    use crate::execution::execute_ir_request;

    let rules = collect_rule_pairs(session, rules_term);
    if rules.is_empty() {
        return Ok(push_semantic(session, SemanticOperator::ReplaceAll, vec![expr, rules_term]));
    }
    let mut cur = expr;
    for (lhs, rhs) in rules {
        cur = crate::execution::builtins::patterns::replace_literal(session, cur, lhs, rhs);
    }
    match execute_ir_request(session, AthenaRequest::Term(cur)) {
        Ok(result_id) => Ok(session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(cur)),
        Err(_) => Ok(cur),
    }
}

pub(crate) fn try_pythagorean_session(session: &mut Session, expr: TermId) -> Option<TermId> {
    let athena_ir::TermNode::Application { head, arguments } = session.arena.get(expr)?
    else {
        return None;
    };
    if !is_sem(*head, SemanticOperator::Add) || arguments.len() != 2 {
        return None;
    }
    let (a, b) = (arguments[0], arguments[1]);
    if is_trig_sq_session(session, a, UnaryFunction::Sin)
        && is_trig_sq_session(session, b, UnaryFunction::Cos)
        && same_trig_arg_session(session, a, b)
    {
        return Some(session.builder().int(1, Default::default()));
    }
    if is_trig_sq_session(session, a, UnaryFunction::Cos)
        && is_trig_sq_session(session, b, UnaryFunction::Sin)
        && same_trig_arg_session(session, a, b)
    {
        return Some(session.builder().int(1, Default::default()));
    }
    None
}

pub(crate) fn is_trig_sq_session(session: &Session, expr: TermId, function: UnaryFunction) -> bool {
    let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(expr)
    else {
        return false;
    };
    if arguments.len() != 2 || !is_sem(*head, SemanticOperator::Power) {
        return false;
    }
    let exp_is_two = matches!(
        session.arena.get(arguments[1]),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) if n.as_exact_integer() == Some(2)
    );
    if !exp_is_two {
        return false;
    }
    match session.arena.get(arguments[0]) {
        Some(athena_ir::TermNode::Application { head: ApplicationHead::Semantic(op), arguments: inner }) if inner.len() == 1 => {
            op.as_unary() == Some(function)
        }
        _ => false,
    }
}

pub(crate) fn same_trig_arg_session(session: &Session, a: TermId, b: TermId) -> bool {
    let arg = |expr: TermId| -> Option<TermId> {
        let athena_ir::TermNode::Application { arguments, .. } = session.arena.get(expr)?
        else {
            return None;
        };
        if arguments.len() != 2 {
            return None;
        }
        let athena_ir::TermNode::Application { arguments: inner, .. } = session.arena.get(arguments[0])?
        else {
            return None;
        };
        (inner.len() == 1).then_some(inner[0])
    };
    match (arg(a), arg(b)) {
        (Some(x), Some(y)) => session.arena.structural_eq(x, y),
        _ => false,
    }
}

pub(crate) fn nested_list_shape(session: &Session, term: TermId) -> Option<(u64, u64)> {
    let athena_ir::TermNode::Collection { elements: rows, .. } = session.arena.get(term)?
    else {
        return None;
    };
    if rows.is_empty() {
        return Some((0, 0));
    }
    if matches!(session.arena.get(rows[0]), Some(athena_ir::TermNode::Collection { elements: _, .. })) {
        let mut cols: Option<u64> = None;
        for row in rows {
            let cells = match session.arena.get(*row) {
                Some(athena_ir::TermNode::Collection { elements: cells, .. }) => cells.len() as u64,
                _ => return None,
            };
            match cols {
                Some(prev) if prev != cells => return None,
                None => cols = Some(cells),
                _ => {}
            }
        }
        Some((rows.len() as u64, cols.unwrap_or(0)))
    }
    else {
        Some((1, rows.len() as u64))
    }
}

pub(crate) fn term_scalar_rational_session(session: &Session, term: TermId) -> Option<Rational> {
    let n = number_of(session, term)?;
    if let Some(i) = n.as_exact_integer() {
        return Some(Rational::new(Integer::from_i64(i), Integer::one()));
    }
    if let Some(i) = n.as_integer() {
        return Some(Rational::new(clone_integer(i), Integer::one()));
    }
    n.as_rational().map(clone_rational)
}

pub(crate) fn term_to_rational_matrix_session(session: &Session, term: TermId) -> Option<MatrixValue> {
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Collection { elements: rows, .. }) if !rows.is_empty() => {
            if matches!(session.arena.get(rows[0]), Some(athena_ir::TermNode::Collection { elements: _, .. })) {
                let mut data = Vec::new();
                let mut cols: Option<u64> = None;
                for row in rows {
                    let cells = match session.arena.get(*row) {
                        Some(athena_ir::TermNode::Collection { elements: cells, .. }) => cells.clone(),
                        _ => return None,
                    };
                    let c = cells.len() as u64;
                    match cols {
                        Some(prev) if prev != c => return None,
                        None => cols = Some(c),
                        _ => {}
                    }
                    for cell in cells {
                        data.push(term_scalar_rational_session(session, cell)?);
                    }
                }
                MatrixValue::from_rationals_row_major(rows.len() as u64, cols.unwrap_or(0), data).ok()
            }
            else {
                let mut data = Vec::with_capacity(rows.len());
                for cell in rows {
                    data.push(term_scalar_rational_session(session, *cell)?);
                }
                MatrixValue::from_rationals_row_major(1, data.len() as u64, data).ok()
            }
        }
        _ => {
            let r = term_scalar_rational_session(session, term)?;
            MatrixValue::from_rationals_row_major(1, 1, vec![r]).ok()
        }
    }
}

pub(crate) fn rational_to_term_session(session: &mut Session, r: &Rational) -> TermId {
    if r.is_integer() {
        if let Some(i) = r.numerator().to_i64() {
            return session.builder().int(i, Default::default());
        }
    }
    push_number(session, Number::from_rational_normalized(clone_rational(r)))
}

pub(crate) fn matrix_to_nested_list_session(session: &mut Session, m: &MatrixValue) -> Result<TermId> {
    let (rows, cols) = (m.shape().rows, m.shape().cols);
    let mut out = Vec::with_capacity(rows as usize);
    for i in 0..rows {
        let mut row = Vec::with_capacity(cols as usize);
        for j in 0..cols {
            match m.get(i, j)? {
                MatrixEntry::Rational(r) => row.push(rational_to_term_session(session, &r)),
                MatrixEntry::Integer(n) => {
                    if let Some(i64v) = n.to_i64() {
                        row.push(session.builder().int(i64v, Default::default()));
                    }
                    else {
                        row.push(push_number(session, Number::integer(clone_integer(&n))));
                    }
                }
                MatrixEntry::MachineF64(x) => row.push(push_number(session, Number::machine(x))),
            }
        }
        out.push(push_list(session, row));
    }
    Ok(push_list(session, out))
}

/// 投影缺少内置符号项的领域结果（例如精确线性求解）。
pub(crate) fn domain_result_symbolic_term(session: &mut Session, domain: &crate::domains::dispatch::DomainResult) -> Option<TermId> {
    use crate::domains::{
        dispatch::DomainResult,
        linear_algebra::{ExactDetResult, ExactSolveResult, LinearAlgebraResult, LinearAlgebraValue},
    };
    match domain {
        DomainResult::LinearAlgebra(LinearAlgebraResult::Ok { value }) => match value {
            LinearAlgebraValue::Matrix(m) => matrix_to_nested_list_session(session, m).ok(),
            LinearAlgebraValue::ExactSolve(ExactSolveResult { particular: Some(m), .. }) => matrix_to_nested_list_session(session, m).ok(),
            LinearAlgebraValue::ExactDet(ExactDetResult { det, .. }) => Some(rational_to_term_session(session, det)),
            _ => None,
        },
        _ => None,
    }
}
