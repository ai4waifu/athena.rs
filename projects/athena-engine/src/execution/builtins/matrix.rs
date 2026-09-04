//! 矩阵相关 handler 与辅助（legacy `eval_det` / `eval_mldivide` / `eval_dot_binop` 等语义）。

use athena_ir::TermNode;
use athena_numeric::{Integer, Rational};
use athena_types::{Diagnostic, DiagnosticCode, TermId};

use crate::domains::linear_algebra::{MatrixEntry, MatrixValue, SolveDisposition, det_bareiss, solve_exact};

use crate::execution::{
    TermEvaluation,
    builtins::arithmetic::{number_of, power, push_number, times},
    vm::Vm,
};

/// 元素级二元运算种类。
#[derive(Debug, Clone, Copy)]
pub(crate) enum DotOpKind {
    /// 逐元素乘。
    Times,
    /// 逐元素除。
    Divide,
    /// 逐元素幂。
    Power,
}

fn dot_apply(vm: &mut Vm<'_>, kind: DotOpKind, a: TermId, b: TermId) -> TermId {
    match kind {
        DotOpKind::Times => times(vm, &[a, b]),
        DotOpKind::Divide => {
            let one = vm.push_int(-1);
            let inv = power(vm, b, one);
            times(vm, &[a, inv])
        }
        DotOpKind::Power => power(vm, a, b),
    }
}

/// nested List 广播二元运算（legacy `eval_dot_binop`）。
pub(crate) fn dot_binop(vm: &mut Vm<'_>, head: &str, left: TermId, right: TermId, kind: DotOpKind) -> TermEvaluation {
    let op = vm.session.operators.intern(head);
    let l_list = matches!(vm.session.arena.get(left), Some(TermNode::List(_)));
    let r_list = matches!(vm.session.arena.get(right), Some(TermNode::List(_)));
    match (l_list, r_list) {
        (true, true) => {
            let (a, b) = (vm.application_arguments(left).unwrap_or_default(), vm.application_arguments(right).unwrap_or_default());
            if a.len() != b.len() {
                let echo = vm.rebuild_application_operator(op, vec![left, right]);
                return TermEvaluation::invalid(
                    echo,
                    Diagnostic::new(DiagnosticCode::ShapeMismatch)
                        .detail("reason", "elementwise_length_mismatch")
                        .detail("left", a.len().to_string())
                        .detail("right", b.len().to_string()),
                );
            }
            let mut out = Vec::with_capacity(a.len());
            let mut diags = Vec::new();
            for (x, y) in a.iter().zip(b.iter()) {
                let cell = if matches!(vm.session.arena.get(*x), Some(TermNode::List(_)))
                    || matches!(vm.session.arena.get(*y), Some(TermNode::List(_)))
                {
                    dot_binop(vm, head, *x, *y, kind)
                }
                else {
                    TermEvaluation::value(dot_apply(vm, kind, *x, *y))
                };
                if cell.has_error() {
                    let echo = vm.rebuild_application_operator(op, vec![left, right]);
                    diags.extend(cell.diagnostics);
                    return TermEvaluation {
                        term: echo,
                        kind: crate::execution::EvalKind::Unevaluated,
                        status: athena_types::ComputationStatus::Invalid,
                        diagnostics: diags,
                    };
                }
                diags.extend(cell.diagnostics);
                out.push(cell.term);
            }
            TermEvaluation {
                term: vm.push_list(out),
                kind: crate::execution::EvalKind::Value,
                status: athena_types::ComputationStatus::Exact,
                diagnostics: diags,
            }
        }
        (true, false) => {
            let a = vm.application_arguments(left).unwrap_or_default();
            let mut out = Vec::with_capacity(a.len());
            let mut diags = Vec::new();
            for x in a {
                let cell = if matches!(vm.session.arena.get(x), Some(TermNode::List(_))) {
                    dot_binop(vm, head, x, right, kind)
                }
                else {
                    TermEvaluation::value(dot_apply(vm, kind, x, right))
                };
                if cell.has_error() {
                    let echo = vm.rebuild_application_operator(op, vec![left, right]);
                    diags.extend(cell.diagnostics);
                    return TermEvaluation {
                        term: echo,
                        kind: crate::execution::EvalKind::Unevaluated,
                        status: athena_types::ComputationStatus::Invalid,
                        diagnostics: diags,
                    };
                }
                diags.extend(cell.diagnostics);
                out.push(cell.term);
            }
            TermEvaluation {
                term: vm.push_list(out),
                kind: crate::execution::EvalKind::Value,
                status: athena_types::ComputationStatus::Exact,
                diagnostics: diags,
            }
        }
        (false, true) => {
            let b = vm.application_arguments(right).unwrap_or_default();
            let mut out = Vec::with_capacity(b.len());
            let mut diags = Vec::new();
            for y in b {
                let cell = if matches!(vm.session.arena.get(y), Some(TermNode::List(_))) {
                    dot_binop(vm, head, left, y, kind)
                }
                else {
                    TermEvaluation::value(dot_apply(vm, kind, left, y))
                };
                if cell.has_error() {
                    let echo = vm.rebuild_application_operator(op, vec![left, right]);
                    diags.extend(cell.diagnostics);
                    return TermEvaluation {
                        term: echo,
                        kind: crate::execution::EvalKind::Unevaluated,
                        status: athena_types::ComputationStatus::Invalid,
                        diagnostics: diags,
                    };
                }
                diags.extend(cell.diagnostics);
                out.push(cell.term);
            }
            TermEvaluation {
                term: vm.push_list(out),
                kind: crate::execution::EvalKind::Value,
                status: athena_types::ComputationStatus::Exact,
                diagnostics: diags,
            }
        }
        (false, false) => {
            if number_of(vm, left).is_some() && number_of(vm, right).is_some() {
                return TermEvaluation::value(dot_apply(vm, kind, left, right));
            }
            TermEvaluation::unevaluated(vm.rebuild_application_operator(op, vec![left, right]))
        }
    }
}

pub(crate) fn h_zeros(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    matrix_fill(vm, "Zeros", args, 0)
}

pub(crate) fn h_ones(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    matrix_fill(vm, "Ones", args, 1)
}

fn matrix_fill(vm: &mut Vm<'_>, head: &str, args: &[TermId], fill: i64) -> TermEvaluation {
    let Some((rows, cols)) = parse_matrix_dims(vm, args)
    else {
        return TermEvaluation::unevaluated(vm.push_application(head, args.to_vec()));
    };
    let n = match rows.checked_mul(cols) {
        Some(v) => v as usize,
        None => {
            return TermEvaluation::invalid(vm.push_list(vec![]), Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "dims_overflow"));
        }
    };
    let fill_r = Rational::new(Integer::from_i64(fill), Integer::one());
    let mut data = Vec::with_capacity(n);
    for _ in 0..n {
        data.push(crate::runtime::values::numeric_clone::clone_rational(&fill_r));
    }
    match MatrixValue::from_rationals_row_major(rows, cols, data) {
        Ok(m) => match matrix_to_nested_list(vm, &m) {
            Ok(term) => TermEvaluation::value(term),
            Err(d) => TermEvaluation::invalid(vm.push_list(vec![]), d),
        },
        Err(d) => TermEvaluation::invalid(vm.push_list(vec![]), d),
    }
}

pub(crate) fn h_eye(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    // `EvalOp` 传入已求值维度参数（与 `Zeros` / `Ones` 同形），不是 EvalRaw root。
    let Some((rows, cols)) = parse_matrix_dims(vm, args)
    else {
        return TermEvaluation::unevaluated(vm.push_application("Eye", args.to_vec()));
    };
    let n = match rows.checked_mul(cols) {
        Some(v) => v as usize,
        None => {
            return TermEvaluation::invalid(
                vm.push_application("Eye", args.to_vec()),
                Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "dims_overflow"),
            );
        }
    };
    let zero = Rational::new(Integer::from_i64(0), Integer::one());
    let mut data = Vec::with_capacity(n);
    for _ in 0..n {
        data.push(crate::runtime::values::numeric_clone::clone_rational(&zero));
    }
    let one = Rational::new(Integer::from_i64(1), Integer::one());
    let diag = rows.min(cols);
    for i in 0..diag {
        data[(i * cols + i) as usize] = crate::runtime::values::numeric_clone::clone_rational(&one);
    }
    let echo = vm.push_application("Eye", args.to_vec());
    match MatrixValue::from_rationals_row_major(rows, cols, data) {
        Ok(m) => match matrix_to_nested_list(vm, &m) {
            Ok(term) => TermEvaluation::value(term),
            Err(d) => TermEvaluation::invalid(echo, d),
        },
        Err(d) => TermEvaluation::invalid(echo, d),
    }
}

pub(crate) fn h_size(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    match nested_list_shape(vm, args[0]) {
        Some((rows, cols)) => {
            let r = vm.push_int(rows as i64);
            let c = vm.push_int(cols as i64);
            TermEvaluation::value(vm.push_list(vec![r, c]))
        }
        None => TermEvaluation::unevaluated(vm.push_application("Size", vec![args[0]])),
    }
}

pub(crate) fn h_det(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    let echo = vm.push_application("Det", vec![args[0]]);
    let Some(m) = term_to_rational_matrix(vm, args[0])
    else {
        return TermEvaluation::unevaluated(echo);
    };
    match det_bareiss(&m) {
        Ok(r) => TermEvaluation::value(rational_to_term(vm, &r.det)),
        Err(d) => TermEvaluation::invalid(echo, d),
    }
}

pub(crate) fn h_linear_solve(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    let echo = vm.push_application("LinearSolve", vec![args[0], args[1]]);
    mldivide(vm, "LinearSolve", args[0], args[1], echo)
}

pub(crate) fn h_solve(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    super::domains::solve(vm, args[0], args[1])
}

/// exact `A\b`（legacy `eval_mldivide`）。echo 由调用方构造（保持 head 名）。
pub(crate) fn mldivide(vm: &mut Vm<'_>, head: &str, a: TermId, b: TermId, echo: TermId) -> TermEvaluation {
    let Some(am) = term_to_rational_matrix(vm, a)
    else {
        return TermEvaluation::unevaluated(echo);
    };
    let Some(bm) = term_to_rational_matrix(vm, b)
    else {
        return TermEvaluation::unevaluated(echo);
    };
    match solve_exact(&am, &bm) {
        Ok(sol) if sol.disposition == SolveDisposition::Unique => match sol.particular {
            Some(x) => match matrix_to_nested_list(vm, &x) {
                Ok(term) => TermEvaluation::value(term),
                Err(d) => TermEvaluation::invalid(echo, d),
            },
            None => TermEvaluation::invalid(echo, Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", head)),
        },
        Ok(sol) => {
            let detail = match sol.disposition {
                crate::domains::linear_algebra::SolveDisposition::Inconsistent => "inconsistent",
                crate::domains::linear_algebra::SolveDisposition::Infinite { .. } => "underdetermined",
                crate::domains::linear_algebra::SolveDisposition::Unique => "unique",
                crate::domains::linear_algebra::SolveDisposition::Singular => "singular",
                crate::domains::linear_algebra::SolveDisposition::ResourceLimited => "resource_limited",
            };
            TermEvaluation::invalid(echo, Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", head).detail("reason", detail))
        }
        Err(d) => TermEvaluation::invalid(echo, d),
    }
}

/// MATLAB-style `sum`：向量 → 标量和；矩阵 → 各列之和（行向量）。
pub(crate) fn array_sum(vm: &mut Vm<'_>, arg: TermId) -> TermEvaluation {
    let echo = vm.push_application("Sum", vec![arg]);
    let Some(m) = term_to_rational_matrix(vm, arg)
    else {
        return TermEvaluation::unevaluated(echo);
    };
    let (rows, cols) = (m.shape().rows, m.shape().cols);
    let mut entry_q = |i: u64, j: u64| -> std::result::Result<Rational, Diagnostic> {
        match m.get(i, j)? {
            MatrixEntry::Rational(r) => Ok(r),
            MatrixEntry::Integer(n) => Ok(Rational::from_integer(n)),
            MatrixEntry::MachineF64(_) => Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "sum_requires_exact")),
        }
    };
    if rows == 1 || cols == 1 {
        let mut acc = Rational::new(Integer::from_i64(0), Integer::one());
        for i in 0..rows {
            for j in 0..cols {
                match entry_q(i, j) {
                    Ok(r) => acc = acc.add(&r),
                    Err(d) => return TermEvaluation::invalid(echo, d),
                }
            }
        }
        return TermEvaluation::value(rational_to_term(vm, &acc));
    }
    let mut out = Vec::with_capacity(cols as usize);
    for j in 0..cols {
        let mut acc = Rational::new(Integer::from_i64(0), Integer::one());
        for i in 0..rows {
            match entry_q(i, j) {
                Ok(r) => acc = acc.add(&r),
                Err(d) => return TermEvaluation::invalid(echo, d),
            }
        }
        out.push(rational_to_term(vm, &acc));
    }
    TermEvaluation::value(vm.push_list(out))
}

// ---- 矩阵 ↔ arena 辅助 ----

fn term_scalar_rational(vm: &Vm<'_>, term: TermId) -> Option<Rational> {
    let n = number_of(vm, term)?;
    if let Some(i) = n.as_exact_integer() {
        return Some(Rational::new(Integer::from_i64(i), Integer::one()));
    }
    if let Some(i) = n.as_integer() {
        return Some(Rational::new(crate::runtime::values::numeric_clone::clone_integer(i), Integer::one()));
    }
    n.as_rational().map(crate::runtime::values::numeric_clone::clone_rational)
}

/// nested-list 矩阵 → `MatrixValue`（legacy `term_to_rational_matrix`）。
pub(crate) fn term_to_rational_matrix(vm: &Vm<'_>, term: TermId) -> Option<MatrixValue> {
    match vm.session.arena.get(term) {
        Some(TermNode::List(rows)) if !rows.is_empty() => {
            if matches!(vm.session.arena.get(rows[0]), Some(TermNode::List(_))) {
                let mut data = Vec::new();
                let mut cols: Option<u64> = None;
                for row in rows {
                    let cells = match vm.session.arena.get(*row) {
                        Some(TermNode::List(cells)) => cells.clone(),
                        _ => return None,
                    };
                    let c = cells.len() as u64;
                    match cols {
                        Some(prev) if prev != c => return None,
                        None => cols = Some(c),
                        _ => {}
                    }
                    for cell in cells {
                        data.push(term_scalar_rational(vm, cell)?);
                    }
                }
                MatrixValue::from_rationals_row_major(rows.len() as u64, cols.unwrap_or(0), data).ok()
            }
            else {
                let mut data = Vec::with_capacity(rows.len());
                for cell in rows {
                    data.push(term_scalar_rational(vm, *cell)?);
                }
                MatrixValue::from_rationals_row_major(1, data.len() as u64, data).ok()
            }
        }
        _ => {
            let r = term_scalar_rational(vm, term)?;
            MatrixValue::from_rationals_row_major(1, 1, vec![r]).ok()
        }
    }
}

/// `MatrixValue` → nested List（legacy `matrix_to_nested_list`）。
pub(crate) fn matrix_to_nested_list(vm: &mut Vm<'_>, m: &MatrixValue) -> std::result::Result<TermId, Diagnostic> {
    let (rows, cols) = (m.shape().rows, m.shape().cols);
    let mut out = Vec::with_capacity(rows as usize);
    for i in 0..rows {
        let mut row = Vec::with_capacity(cols as usize);
        for j in 0..cols {
            match m.get(i, j)? {
                MatrixEntry::Rational(r) => row.push(rational_to_term(vm, &r)),
                MatrixEntry::Integer(n) => {
                    if let Some(i64v) = n.to_i64() {
                        row.push(vm.push_int(i64v));
                    }
                    else {
                        row.push(push_number(vm, athena_numeric::Number::integer(crate::runtime::values::numeric_clone::clone_integer(&n))));
                    }
                }
                MatrixEntry::MachineF64(x) => row.push(push_number(vm, athena_numeric::Number::machine(x))),
            }
        }
        out.push(vm.push_list(row));
    }
    Ok(vm.push_list(out))
}

/// `Rational` → 整数或精确有理数原子（legacy `rational_to_term`）。
pub(crate) fn rational_to_term(vm: &mut Vm<'_>, r: &Rational) -> TermId {
    if r.is_integer() {
        if let Some(i) = r.numerator().to_i64() {
            return vm.push_int(i);
        }
    }
    push_number(vm, athena_numeric::Number::from_rational_normalized(crate::runtime::values::numeric_clone::clone_rational(r)))
}

/// `n` / `m,n` 非负整数维度（legacy `parse_matrix_dims`）。
fn parse_matrix_dims(vm: &Vm<'_>, args: &[TermId]) -> Option<(u64, u64)> {
    let as_dim = |t: TermId| -> Option<u64> {
        let n = number_of(vm, t)?.as_exact_integer()?;
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

/// nested-list 矩阵 / 行向量形状（legacy `nested_list_shape`）。
pub(crate) fn nested_list_shape(vm: &Vm<'_>, term: TermId) -> Option<(u64, u64)> {
    let Some(TermNode::List(rows)) = vm.session.arena.get(term)
    else {
        return None;
    };
    if rows.is_empty() {
        return Some((0, 0));
    }
    if matches!(vm.session.arena.get(rows[0]), Some(TermNode::List(_))) {
        let mut cols: Option<u64> = None;
        for row in rows {
            let cells = match vm.session.arena.get(*row) {
                Some(TermNode::List(cells)) => cells.len() as u64,
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
