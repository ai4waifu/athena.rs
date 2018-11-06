//! 单项式指数算术（checked；禁止 saturate / wrap）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

/// 两指数向量逐项相加（长度须一致）。
pub(crate) fn add_exponent_vectors(lhs: &[u32], rhs: &[u32]) -> Result<Vec<u32>> {
    lhs.iter().zip(rhs.iter()).map(|(&a, &b)| add_exponents(a, b)).collect()
}

/// 单变量指数 checked 加法。
pub(crate) fn add_exponents(a: u32, b: u32) -> Result<u32> {
    a.checked_add(b).ok_or_else(degree_overflow)
}

fn degree_overflow() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::PolynomialDegreeOverflow).detail("domain", "polynomial").detail("operation", "exponent_add")
}
