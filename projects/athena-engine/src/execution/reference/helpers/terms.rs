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

/// 编译并再求值一项。失败与取消/预算诊断向上传播，禁止吞成原项。
///
/// 嵌套入口经 Session [`crate::runtime::session::SharedExecutionControl`] 继承取消与剩余预算。
pub(crate) fn re_eval_term(session: &mut Session, term: TermId) -> Result<TermId> {
    let result_id = execute_ir_request(session, AthenaRequest::Term(term))?;
    session.results.require_symbolic_term(result_id)
}

/// `Unary(f)[arg]` — 精确特殊值 / 精确三角折叠 / machine 实数折叠，否则残差。
pub(crate) fn evaluate_special_unary_terms(session: &mut Session, op: SemanticOperator, terms: Vec<TermId>) -> Result<TermId> {
    if let Some(uf) = op.as_unary() {
        if terms.len() == 1 {
            let arg = terms[0];
            if let Some(exact) = eval_exact_special_unary(session, uf, arg) {
                return Ok(exact);
            }
            if let Some(exact) = eval_trig_exact_session(session, uf, arg) {
                return Ok(exact);
            }
            // 仅当参数已是 machine 实数时折叠。禁止把精确 `Sin[1]` 经 `f64` 自动 `N`。
            if let Some(x) = number_of(session, arg).and_then(|n| n.as_machine_f64()) {
                let y = match uf {
                    UnaryFunction::Sin => x.sin(),
                    UnaryFunction::Cos => x.cos(),
                    UnaryFunction::Tan => x.tan(),
                    UnaryFunction::Exp => x.exp(),
                    UnaryFunction::Log => x.ln(),
                    _ => f64::NAN,
                };
                if y.is_finite() {
                    return Ok(push_number(session, Number::machine(y)));
                }
            }
        }
    }
    Ok(push_semantic(session, op, terms))
}

/// Exact kernel specials: `Exp[0]→1`, `Log[1]→0` (and machine equivalents already covered below).
fn eval_exact_special_unary(session: &mut Session, function: UnaryFunction, arg: TermId) -> Option<TermId> {
    let n = number_of(session, arg)?;
    match function {
        UnaryFunction::Exp if n.is_zero() => Some(session.builder().int(1, Default::default())),
        UnaryFunction::Log if n.is_one() => Some(session.builder().int(0, Default::default())),
        _ => None,
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
pub(crate) fn evaluate_rule_terms(session: &mut Session, op: SemanticOperator, lhs: TermId, rhs: TermId) -> Result<TermId> {
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
    let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(list)
    else {
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

/// `ReplaceAll[expr, rules]` — 字面替换后再走 `re_eval_term`。
pub(crate) fn evaluate_replace_all_terms(session: &mut Session, expr: TermId, rules_term: TermId) -> Result<TermId> {
    let rules = collect_rule_pairs(session, rules_term);
    if rules.is_empty() {
        return Ok(push_semantic(session, SemanticOperator::ReplaceAll, vec![expr, rules_term]));
    }
    let mut cur = expr;
    for (lhs, rhs) in rules {
        cur = crate::execution::builtins::patterns::replace_literal(session, cur, lhs, rhs);
    }
    re_eval_term(session, cur)
}

/// `Simplify[expr]` — algebraic rewrite of an already-produced value.
///
/// Does **not** re-evaluate against ambient Own / session definitions. Re-eval would
/// reinterpret a computation result as a fresh source expression (for example turning a
/// free `x` left by `DynamicScope` into an outer Own binding).
pub(crate) fn evaluate_simplify_terms(session: &mut Session, expr: TermId) -> Result<TermId> {
    if let Some(one) = try_pythagorean_session(session, expr) {
        return Ok(one);
    }
    Ok(expr)
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
    // 1×n 行向量投影为平坦 List，匹配 MATLAB / Mathematica 向量表面。
    if rows == 1 {
        let mut row = Vec::with_capacity(cols as usize);
        for j in 0..cols {
            row.push(matrix_entry_to_term_session(session, m, 0, j)?);
        }
        return Ok(push_list(session, row));
    }
    let mut out = Vec::with_capacity(rows as usize);
    for i in 0..rows {
        let mut row = Vec::with_capacity(cols as usize);
        for j in 0..cols {
            row.push(matrix_entry_to_term_session(session, m, i, j)?);
        }
        out.push(push_list(session, row));
    }
    Ok(push_list(session, out))
}

pub(crate) fn matrix_entry_to_term_session(session: &mut Session, m: &MatrixValue, row: u64, col: u64) -> Result<TermId> {
    Ok(match m.get(row, col)? {
        MatrixEntry::Rational(r) => rational_to_term_session(session, &r),
        MatrixEntry::Integer(n) => {
            if let Some(i64v) = n.to_i64() {
                session.builder().int(i64v, Default::default())
            }
            else {
                push_number(session, Number::integer(clone_integer(&n)))
            }
        }
        MatrixEntry::MachineF64(x) => push_number(session, Number::machine(x)),
    })
}

/// `Dot` 结果：`1×1` → 标量，行/列向量 → 平坦 List，否则嵌套矩阵。
pub(crate) fn matrix_to_dot_term_session(session: &mut Session, m: &MatrixValue) -> Result<TermId> {
    let (rows, cols) = (m.shape().rows, m.shape().cols);
    if rows == 1 && cols == 1 {
        return matrix_entry_to_term_session(session, m, 0, 0);
    }
    if cols == 1 {
        let mut out = Vec::with_capacity(rows as usize);
        for i in 0..rows {
            out.push(matrix_entry_to_term_session(session, m, i, 0)?);
        }
        return Ok(push_list(session, out));
    }
    if rows == 1 {
        let mut out = Vec::with_capacity(cols as usize);
        for j in 0..cols {
            out.push(matrix_entry_to_term_session(session, m, 0, j)?);
        }
        return Ok(push_list(session, out));
    }
    matrix_to_nested_list_session(session, m)
}

/// 投影缺少内置符号项的领域结果（例如精确线性求解）。
pub(crate) fn domain_result_symbolic_term(session: &mut Session, domain: &crate::domains::dispatch::DomainResult) -> Option<TermId> {
    use crate::domains::{
        dispatch::DomainResult,
        linear_algebra::{
            ExactDetResult, ExactNormResult, ExactRankResult, ExactRrefResult, ExactSolveResult, ExactTraceResult, LinearAlgebraResult,
            LinearAlgebraValue,
        },
    };
    match domain {
        DomainResult::LinearAlgebra(LinearAlgebraResult::Ok { value }) => match value {
            LinearAlgebraValue::Matrix(m) => matrix_to_nested_list_session(session, m).ok(),
            LinearAlgebraValue::Dot(m) => matrix_to_dot_term_session(session, m).ok(),
            LinearAlgebraValue::ExactSolve(ExactSolveResult { particular: Some(m), .. }) => matrix_to_nested_list_session(session, m).ok(),
            LinearAlgebraValue::ExactDet(ExactDetResult { det, .. }) => Some(rational_to_term_session(session, det)),
            LinearAlgebraValue::ExactTrace(ExactTraceResult { value, .. }) => Some(rational_to_term_session(session, value)),
            LinearAlgebraValue::ExactNorm(ExactNormResult { value, .. }) => Some(rational_to_term_session(session, value)),
            LinearAlgebraValue::ExactRank(ExactRankResult { rank, .. }) => {
                let Ok(n) = i64::try_from(*rank) else {
                    return None;
                };
                Some(session.builder().int(n, Default::default()))
            }
            LinearAlgebraValue::ExactRref(ExactRrefResult { matrix, .. }) => matrix_to_nested_list_session(session, matrix).ok(),
            _ => None,
        },
        _ => None,
    }
}

/// 领域请求的残差 Extension 回声（未绑定矩阵等 Own 投影）。
pub(crate) fn domain_request_residual_term(session: &mut Session, domain: &crate::domains::dispatch::DomainRequest) -> Option<TermId> {
    use crate::domains::{dispatch::DomainRequest, linear_algebra::LinearAlgebraRequest};
    match domain {
        DomainRequest::LinearAlgebra(req) => linear_algebra_request_residual_term(session, req),
        _ => None,
    }
}

/// 线性代数 Err 是否因矩阵绑定缺失（可 Own 回声，而非输入形状硬失败）。
pub(crate) fn linear_algebra_missing_binding(domain: &crate::domains::dispatch::DomainResult) -> bool {
    use athena_types::DiagnosticValue;
    use crate::domains::{dispatch::DomainResult, linear_algebra::LinearAlgebraResult};
    match domain {
        DomainResult::LinearAlgebra(LinearAlgebraResult::Err { diagnostic }) => {
            matches!(diagnostic.details.get("reason"), Some(DiagnosticValue::Text(reason)) if reason == "missing_matrix_binding")
        }
        _ => false,
    }
}

fn linear_algebra_request_residual_term(
    session: &mut Session,
    request: &crate::domains::linear_algebra::LinearAlgebraRequest,
) -> Option<TermId> {
    use crate::domains::linear_algebra::{LinearAlgebraRequest, MatrixOperand};
    use crate::runtime::values::arena::push_extension;

    let matrix_op_term = |session: &mut Session, op: MatrixOperand| -> Option<TermId> {
        match op {
            MatrixOperand::Binding(symbol) => Some(session.builder().symbol_id(symbol, Default::default())),
            MatrixOperand::Object(matrix_ref) => {
                let matrix = session.matrix_objects.resolve_owning(matrix_ref)?;
                matrix_to_nested_list_session(session, &matrix).ok()
            }
        }
    };

    let (head, args) = match request {
        LinearAlgebraRequest::Transpose { matrix } => ("Transpose", vec![matrix_op_term(session, *matrix)?]),
        LinearAlgebraRequest::Det { matrix } => ("Det", vec![matrix_op_term(session, *matrix)?]),
        LinearAlgebraRequest::Rank { matrix } => ("MatrixRank", vec![matrix_op_term(session, *matrix)?]),
        LinearAlgebraRequest::Inverse { matrix } => ("Inverse", vec![matrix_op_term(session, *matrix)?]),
        LinearAlgebraRequest::Trace { matrix } => ("Tr", vec![matrix_op_term(session, *matrix)?]),
        LinearAlgebraRequest::Rref { matrix } => ("RowReduce", vec![matrix_op_term(session, *matrix)?]),
        LinearAlgebraRequest::Norm { matrix } => ("Norm", vec![matrix_op_term(session, *matrix)?]),
        LinearAlgebraRequest::NullSpace { matrix } => ("NullSpace", vec![matrix_op_term(session, *matrix)?]),
        LinearAlgebraRequest::Solve { a, b } => ("LinearSolve", vec![matrix_op_term(session, *a)?, matrix_op_term(session, *b)?]),
        LinearAlgebraRequest::MatMul { lhs, rhs } => ("Dot", vec![matrix_op_term(session, *lhs)?, matrix_op_term(session, *rhs)?]),
        LinearAlgebraRequest::Hadamard { lhs, rhs } => ("DotTimes", vec![matrix_op_term(session, *lhs)?, matrix_op_term(session, *rhs)?]),
        LinearAlgebraRequest::Dot { lhs, rhs } => ("Dot", vec![matrix_op_term(session, *lhs)?, matrix_op_term(session, *rhs)?]),
        LinearAlgebraRequest::Cross { lhs, rhs } => ("Cross", vec![matrix_op_term(session, *lhs)?, matrix_op_term(session, *rhs)?]),
        LinearAlgebraRequest::Index { .. } => return None,
    };
    let op = session.extensions.intern(head);
    Some(push_extension(session, op, args))
}

/// 将 `ValueStore` 载荷投影为可渲染符号项（矩阵 DomainObject → 嵌套 List）。
pub(crate) fn symbolic_term_from_value_id(session: &mut Session, value_id: athena_types::ValueId) -> Result<TermId> {
    use crate::runtime::RuntimeValue;
    enum Copied {
        Matrix(crate::domains::linear_algebra::MatrixRef),
        Term(TermId),
        Boolean(bool),
        Null,
    }
    let copied = match session.values.get(value_id) {
        Some(RuntimeValue::Matrix(matrix)) => Copied::Matrix(*matrix),
        Some(RuntimeValue::SymbolicTerm(term)) => Copied::Term(*term),
        Some(RuntimeValue::Boolean(v)) => Copied::Boolean(*v),
        Some(RuntimeValue::Null) => Copied::Null,
        Some(RuntimeValue::Domain(_)) => {
            return Err(diag("value_domain_needs_publish_result"));
        }
        None => return Err(diag("value_id_missing")),
    };
    match copied {
        Copied::Matrix(matrix_ref) => {
            let matrix = session.matrix_objects.resolve_owning(matrix_ref).ok_or_else(|| diag("matrix_ref_missing"))?;
            matrix_to_nested_list_session(session, &matrix)
        }
        Copied::Term(term) => Ok(term),
        Copied::Boolean(v) => Ok(session.builder().boolean(v, Default::default())),
        Copied::Null => Ok(session.builder().null(Default::default())),
    }
}
