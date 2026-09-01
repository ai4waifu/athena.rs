//! 多项式域分派（骨架：尚未实现的请求 → `UnsupportedOperation`）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{request::PolynomialRequest, value::PolynomialDomainValue};

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

/// 执行多项式域请求。
pub fn execute_polynomial(request: PolynomialRequest) -> PolynomialResult {
    let op = match &request {
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
