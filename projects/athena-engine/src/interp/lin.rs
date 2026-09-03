//! 矩阵相关 handler 与辅助（legacy `eval_det` / `eval_mldivide` / `eval_dot_binop` 等语义）。

use athena_ir::TermKind;
use athena_numeric::{Integer, Rational};
use athena_types::{Diagnostic, DiagnosticCode, TermId};

use crate::linear_algebra::{MatrixEntry, MatrixValue, SolveDisposition, det_bareiss, solve_exact};

use super::{
    Outcome,
    arith::{number_of, power, push_number, times},
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
pub(crate) fn dot_binop(vm: &mut Vm<'_>, head: &str, left: TermId, right: TermId, kind: DotOpKind) -> Outcome {
    let op = vm.session.operators.intern(head);
    let l_list = matches!(vm.session.arena.get(left), Some(TermKind::List(_)));
    let r_list = matches!(vm.session.arena.get(right), Some(TermKind::List(_)));
    match (l_list, r_list) {
        (true, true) => {
            let (a, b) = (vm.app_args(left).unwrap_or_default(), vm.app_args(right).unwrap_or_default());
            if a.len() != b.len() {
                let echo = vm.rebuild_app_op(op, vec![left, right]);
                return Outcome::invalid(
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
                let cell = if matches!(vm.session.arena.get(*x), Some(TermKind::List(_)))
                    || matches!(vm.session.arena.get(*y), Some(TermKind::List(_)))
                {
                    dot_binop(vm, head, *x, *y, kind)
                }
                else {
                    Outcome::value(dot_apply(vm, kind, *x, *y))
                };
                if cell.has_error() {
                    let echo = vm.rebuild_app_op(op, vec![left, right]);
                    diags.extend(cell.diagnostics);
                    return Outcome {
                        term: echo,
                        kind: super::EvalKind::Unevaluated,
                        status: athena_types::ComputationStatus::Invalid,
                        diagnostics: diags,
                    };
                }
                diags.extend(cell.diagnostics);
                out.push(cell.term);
            }
            Outcome {
                term: vm.push_list(out),
                kind: super::EvalKind::Value,
                status: athena_types::ComputationStatus::Exact,
                diagnostics: diags,
            }
        }
        (true, false) => {
            let a = vm.app_args(left).unwrap_or_default();
            let mut out = Vec::with_capacity(a.len());
            let mut diags = Vec::new();
            for x in a {
                let cell = if matches!(vm.session.arena.get(x), Some(TermKind::List(_))) {
                    dot_binop(vm, head, x, right, kind)
                }
                else {
                    Outcome::value(dot_apply(vm, kind, x, right))
                };
                if cell.has_error() {
                    let echo = vm.rebuild_app_op(op, vec![left, right]);
                    diags.extend(cell.diagnostics);
                    return Outcome {
                        term: echo,
                        kind: super::EvalKind::Unevaluated,
                        status: athena_types::ComputationStatus::Invalid,
                        diagnostics: diags,
                    };
                }
                diags.extend(cell.diagnostics);
                out.push(cell.term);
            }
            Outcome {
                term: vm.push_list(out),
                kind: super::EvalKind::Value,
                status: athena_types::ComputationStatus::Exact,
                diagnostics: diags,
            }
        }
        (false, true) => {
            let b = vm.app_args(right).unwrap_or_default();
            let mut out = Vec::with_capacity(b.len());
            let mut diags = Vec::new();
            for y in b {
                let cell = if matches!(vm.session.arena.get(y), Some(TermKind::List(_))) {
                    dot_binop(vm, head, left, y, kind)
                }
                else {
                    Outcome::value(dot_apply(vm, kind, left, y))
                };
                if cell.has_error() {
                    let echo = vm.rebuild_app_op(op, vec![left, right]);
                    diags.extend(cell.diagnostics);
                    return Outcome {
                        term: echo,
                        kind: super::EvalKind::Unevaluated,
                        status: athena_types::ComputationStatus::Invalid,
                        diagnostics: diags,
                    };
                }
                diags.extend(cell.diagnostics);
                out.push(cell.term);
            }
            Outcome {
                term: vm.push_list(out),
                kind: super::EvalKind::Value,
                status: athena_types::ComputationStatus::Exact,
                diagnostics: diags,
            }
        }
        (false, false) => {
            if number_of(vm, left).is_some() && number_of(vm, right).is_some() {
                return Outcome::value(dot_apply(vm, kind, left, right));
            }
            Outcome::unevaluated(vm.rebuild_app_op(op, vec![left, right]))
        }
    }
}

pub(crate) fn h_zeros(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    matrix_fill(vm, "Zeros", args, 0)
}

pub(crate) fn h_ones(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    matrix_fill(vm, "Ones", args, 1)
}

fn matrix_fill(vm: &mut Vm<'_>, head: &str, args: &[TermId], fill: i64) -> Outcome {
    let Some((rows, cols)) = parse_matrix_dims(vm, args)
    else {
        return Outcome::unevaluated(vm.push_app(head, args.to_vec()));
    };
    let n = match rows.checked_mul(cols) {
        Some(v) => v as usize,
        None => {
            return Outcome::invalid(
                vm.push_list(vec![]),
                Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "dims_overflow"),
            );
        }
    };
    let fill_r = Rational::new(Integer::from_i64(fill), Integer::one());
    let mut data = Vec::with_capacity(n);
    for _ in 0..n {
        data.push(crate::numeric_clone::clone_rational(&fill_r));
    }
    match MatrixValue::from_rationals_row_major(rows, cols, data) {
        Ok(m) => match matrix_to_nested_list(vm, &m) {
            Ok(term) => Outcome::value(term),
            Err(d) => Outcome::invalid(vm.push_list(vec![]), d),
        },
        Err(d) => Outcome::invalid(vm.push_list(vec![]), d),
    }
}

pub(crate) fn h_eye(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    let root = operands[0];
    let name = vm.head_name(root).unwrap_or_default();
    let args = vm.app_args(root).unwrap_or_default();
    let Some((rows, cols)) = parse_matrix_dims(vm, &args)
    else {
        let op = vm.session.operators.intern(&name);
        return Outcome::unevaluated(vm.rebuild_app_op(op, args));
    };
    let n = match rows.checked_mul(cols) {
        Some(v) => v as usize,
        None => {
            let op = vm.session.operators.intern(&name);
            let echo = vm.rebuild_app_op(op, args);
            return Outcome::invalid(echo, Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "dims_overflow"));
        }
    };
    let zero = Rational::new(Integer::from_i64(0), Integer::one());
    let mut data = Vec::with_capacity(n);
    for _ in 0..n {
        data.push(crate::numeric_clone::clone_rational(&zero));
    }
    let one = Rational::new(Integer::from_i64(1), Integer::one());
    let diag = rows.min(cols);
    for i in 0..diag {
        data[(i * cols + i) as usize] = crate::numeric_clone::clone_rational(&one);
    }
    let op = vm.session.operators.intern(&name);
    let echo = vm.rebuild_app_op(op, args);
    match MatrixValue::from_rationals_row_major(rows, cols, data) {
        Ok(m) => match matrix_to_nested_list(vm, &m) {
            Ok(term) => Outcome::value(term),
            Err(d) => Outcome::invalid(echo, d),
        },
        Err(d) => Outcome::invalid(echo, d),
    }
}

pub(crate) fn h_size(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    match nested_list_shape(vm, args[0]) {
        Some((rows, cols)) => {
            let r = vm.push_int(rows as i64);
            let c = vm.push_int(cols as i64);
            Outcome::value(vm.push_list(vec![r, c]))
        }
        None => Outcome::unevaluated(vm.push_app("Size", vec![args[0]])),
    }
}

pub(crate) fn h_det(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let echo = vm.push_app("Det", vec![args[0]]);
    let Some(m) = term_to_rational_matrix(vm, args[0])
    else {
        return Outcome::unevaluated(echo);
    };
    match det_bareiss(&m) {
        Ok(r) => Outcome::value(rational_to_term(vm, &r.det)),
        Err(d) => Outcome::invalid(echo, d),
    }
}

pub(crate) fn h_linear_solve(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let echo = vm.push_app("LinearSolve", vec![args[0], args[1]]);
    mldivide(vm, "LinearSolve", args[0], args[1], echo)
}

pub(crate) fn h_solve(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    super::domain::solve(vm, args[0], args[1])
}

/// exact `A\b`（legacy `eval_mldivide`）。echo 由调用方构造（保持 head 名）。
pub(crate) fn mldivide(vm: &mut Vm<'_>, head: &str, a: TermId, b: TermId, echo: TermId) -> Outcome {
    let Some(am) = term_to_rational_matrix(vm, a)
    else {
        return Outcome::unevaluated(echo);
    };
    let Some(bm) = term_to_rational_matrix(vm, b)
    else {
        return Outcome::unevaluated(echo);
    };
    match solve_exact(&am, &bm) {
        Ok(sol) if sol.disposition == SolveDisposition::Unique => match sol.particular {
            Some(x) => match matrix_to_nested_list(vm, &x) {
                Ok(term) => Outcome::value(term),
                Err(d) => Outcome::invalid(echo, d),
            },
            None => Outcome::invalid(echo, Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", head)),
        },
        Ok(sol) => {
            let detail = match sol.disposition {
                crate::linear_algebra::SolveDisposition::Inconsistent => "inconsistent",
                crate::linear_algebra::SolveDisposition::Infinite { .. } => "underdetermined",
                crate::linear_algebra::SolveDisposition::Unique => "unique",
                crate::linear_algebra::SolveDisposition::Singular => "singular",
                crate::linear_algebra::SolveDisposition::ResourceLimited => "resource_limited",
            };
            Outcome::invalid(
                echo,
                Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", head).detail("reason", detail),
            )
        }
        Err(d) => Outcome::invalid(echo, d),
    }
}

/// MATLAB-style `sum`：向量 → 标量和；矩阵 → 各列之和（行向量）。
pub(crate) fn array_sum(vm: &mut Vm<'_>, arg: TermId) -> Outcome {
    let echo = vm.push_app("Sum", vec![arg]);
    let Some(m) = term_to_rational_matrix(vm, arg)
    else {
        return Outcome::unevaluated(echo);
    };
    let (rows, cols) = (m.shape().rows, m.shape().cols);
    let mut entry_q = |i: u64, j: u64| -> std::result::Result<Rational, Diagnostic> {
        match m.get(i, j)? {
            MatrixEntry::Rational(r) => Ok(r),
            MatrixEntry::Integer(n) => Ok(Rational::from_integer(n)),
            MatrixEntry::MachineF64(_) => {
                Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "sum_requires_exact"))
            }
        }
    };
    if rows == 1 || cols == 1 {
        let mut acc = Rational::new(Integer::from_i64(0), Integer::one());
        for i in 0..rows {
            for j in 0..cols {
                match entry_q(i, j) {
                    Ok(r) => acc = acc.add(&r),
                    Err(d) => return Outcome::invalid(echo, d),
                }
            }
        }
        return Outcome::value(rational_to_term(vm, &acc));
    }
    let mut out = Vec::with_capacity(cols as usize);
    for j in 0..cols {
        let mut acc = Rational::new(Integer::from_i64(0), Integer::one());
        for i in 0..rows {
            match entry_q(i, j) {
                Ok(r) => acc = acc.add(&r),
                Err(d) => return Outcome::invalid(echo, d),
            }
        }
        out.push(rational_to_term(vm, &acc));
    }
    Outcome::value(vm.push_list(out))
}

// ---- 矩阵 ↔ arena 辅助 ----

fn term_scalar_rational(vm: &Vm<'_>, term: TermId) -> Option<Rational> {
    let n = number_of(vm, term)?;
    if let Some(i) = n.as_exact_integer() {
        return Some(Rational::new(Integer::from_i64(i), Integer::one()));
    }
    if let Some(i) = n.as_integer() {
        return Some(Rational::new(crate::numeric_clone::clone_integer(i), Integer::one()));
    }
    n.as_rational().map(crate::numeric_clone::clone_rational)
}

/// nested-list 矩阵 → `MatrixValue`（legacy `term_to_rational_matrix`）。
pub(crate) fn term_to_rational_matrix(vm: &Vm<'_>, term: TermId) -> Option<MatrixValue> {
    match vm.session.arena.get(term) {
        Some(TermKind::List(rows)) if !rows.is_empty() => {
            if matches!(vm.session.arena.get(rows[0]), Some(TermKind::List(_))) {
                let mut data = Vec::new();
                let mut cols: Option<u64> = None;
                for row in rows {
                    let cells = match vm.session.arena.get(*row) {
                        Some(TermKind::List(cells)) => cells.clone(),
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
                        row.push(push_number(vm, athena_numeric::Number::integer(crate::numeric_clone::clone_integer(&n))));
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
    push_number(vm, athena_numeric::Number::from_rational_normalized(crate::numeric_clone::clone_rational(r)))
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
    let Some(TermKind::List(rows)) = vm.session.arena.get(term)
    else {
        return None;
    };
    if rows.is_empty() {
        return Some((0, 0));
    }
    if matches!(vm.session.arena.get(rows[0]), Some(TermKind::List(_))) {
        let mut cols: Option<u64> = None;
        for row in rows {
            let cells = match vm.session.arena.get(*row) {
                Some(TermKind::List(cells)) => cells.len() as u64,
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
