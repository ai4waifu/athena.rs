//! 多项式域分派（Normalize 需 [`RingTable`]；其余仍为骨架）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    canonical::canonicalize_polynomial,
    request::PolynomialRequest,
    ring_table::RingTable,
    value::{PolynomialDomainValue, PolynomialValue},
};

/// 多项式域结果。
#[derive(Debug, Clone, PartialEq)]
pub enum PolynomialResult {
    /// 精确结果。
    Exact {
        /// 值。
        value: PolynomialDomainValue,
    },
    /// 未求值 / 骨架未实现。
    Unevaluated {
        /// 原因。
        reason: Diagnostic,
    },
}

/// 执行多项式域请求（无环表时 Normalize 不可求值）。
pub fn execute_polynomial(request: PolynomialRequest) -> PolynomialResult {
    match &request {
        PolynomialRequest::Normalize { .. } => PolynomialResult::Unevaluated {
            reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "polynomial")
                .detail("operation", "normalize")
                .detail("hint", "use execute_polynomial_with_rings"),
        },
        _ => unsupported_polynomial(&request),
    }
}

/// 在已注册环表上下文中执行多项式域请求。
pub fn execute_polynomial_with_rings(request: PolynomialRequest, rings: &RingTable) -> PolynomialResult {
    match request {
        PolynomialRequest::Normalize { polynomial } => match canonicalize_polynomial(polynomial, rings) {
            Ok(normalized) => PolynomialResult::Exact {
                value: PolynomialDomainValue::Polynomial(PolynomialValue { inner: normalized }),
            },
            Err(reason) => PolynomialResult::Unevaluated { reason },
        },
        other => execute_polynomial(other),
    }
}

fn unsupported_polynomial(request: &PolynomialRequest) -> PolynomialResult {
    let op = match request {
        PolynomialRequest::Normalize { .. } => "normalize",
        PolynomialRequest::Add { .. } => "add",
        PolynomialRequest::Mul { .. } => "mul",
        PolynomialRequest::Div { .. } => "div",
        PolynomialRequest::Gcd { .. } => "gcd",
        PolynomialRequest::Factor { .. } => "factor",
        PolynomialRequest::Groebner { .. } => "groebner",
    };
    PolynomialResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "polynomial").detail("operation", op),
    }
}
