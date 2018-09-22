//! 同余方程（骨架）。

use athena_numeric::Integer;
use athena_types::{Diagnostic, DiagnosticCode};

use super::result::NumberTheoryResult;

/// 线性同余 `a x ≡ b (mod m)`（骨架：尚未求解）。
pub fn solve_linear_congruence(a: &Integer, b: &Integer, m: &Integer) -> NumberTheoryResult {
    let _ = (a, b, m);
    NumberTheoryResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "number_theory")
            .detail("operation", "solve_linear_congruence"),
    }
}
