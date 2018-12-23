//! 矩阵相等：结构 / 精确数学 / 近似。

use athena_types::{Diagnostic, DiagnosticCode};

use super::value::{MatrixEntry, MatrixValue};

/// 相等判定模式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatrixEqualityKind {
    /// 同一 parent、shape、layout、缓冲内容按存储序逐元素相同。
    Structural,
    /// 精确数学相等（忽略 layout；比较数学元素）。
    ExactMathematical,
    /// 近似相等（仅机器路径；给定绝对容差）。
    Approximate {
        /// 绝对容差。
        abs_tol: f64,
    },
}

/// 按指定模式比较。
pub fn matrices_equal(a: &MatrixValue, b: &MatrixValue, kind: MatrixEqualityKind) -> Result<bool, Diagnostic> {
    match kind {
        MatrixEqualityKind::Structural => Ok(a == b),
        MatrixEqualityKind::ExactMathematical => exact_math_equal(a, b),
        MatrixEqualityKind::Approximate { abs_tol } => approx_equal(a, b, abs_tol),
    }
}

fn exact_math_equal(a: &MatrixValue, b: &MatrixValue) -> Result<bool, Diagnostic> {
    if a.shape() != b.shape() {
        return Ok(false);
    }
    if a.parent().element.is_machine() || b.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "exact_eq_rejects_machine"));
    }
    let ar = a.promote_integers_to_rationals()?;
    let br = b.promote_integers_to_rationals()?;
    for r in 0..a.shape().rows {
        for c in 0..a.shape().cols {
            if ar.get(r, c)? != br.get(r, c)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn approx_equal(a: &MatrixValue, b: &MatrixValue, abs_tol: f64) -> Result<bool, Diagnostic> {
    if a.shape() != b.shape() {
        return Ok(false);
    }
    if !a.parent().element.is_machine() || !b.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "approx_eq_requires_machine"));
    }
    for r in 0..a.shape().rows {
        for c in 0..a.shape().cols {
            let av = match a.get(r, c)? {
                MatrixEntry::MachineF64(x) => x,
                _ => unreachable!(),
            };
            let bv = match b.get(r, c)? {
                MatrixEntry::MachineF64(x) => x,
                _ => unreachable!(),
            };
            if (av - bv).abs() > abs_tol {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
