//! 多项式域分派（Normalize 需 [`RingTable`]；其余仍为骨架）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    canonical::canonicalize_polynomial,
    groebner::{compute_elimination_basis, compute_groebner_basis},
    operations::{add_polynomial, mul_polynomial},
    request::PolynomialRequest,
    ring_table::RingTable,
    univariate::{div_univariate, gcd_univariate},
    value::{GroebnerBasisValue, PolynomialDomainValue, PolynomialValue, UnivariateDivisionValue},
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
            Ok(normalized) => {
                PolynomialResult::Exact { value: PolynomialDomainValue::Polynomial(PolynomialValue { inner: normalized }) }
            }
            Err(reason) => PolynomialResult::Unevaluated { reason },
        },
        PolynomialRequest::Add { lhs, rhs } => match add_polynomial(lhs, rhs, rings) {
            Ok(sum) => PolynomialResult::Exact { value: PolynomialDomainValue::Polynomial(PolynomialValue { inner: sum }) },
            Err(reason) => PolynomialResult::Unevaluated { reason },
        },
        PolynomialRequest::Mul { lhs, rhs } => match mul_polynomial(lhs, rhs, rings) {
            Ok(product) => {
                PolynomialResult::Exact { value: PolynomialDomainValue::Polynomial(PolynomialValue { inner: product }) }
            }
            Err(reason) => PolynomialResult::Unevaluated { reason },
        },
        PolynomialRequest::Div { dividend, divisor, policy } => match div_univariate(dividend, divisor, policy, rings) {
            Ok(division) => PolynomialResult::Exact {
                value: PolynomialDomainValue::UnivariateDivision(UnivariateDivisionValue {
                    quotient: PolynomialValue { inner: division.quotient },
                    remainder: PolynomialValue { inner: division.remainder },
                }),
            },
            Err(reason) => PolynomialResult::Unevaluated { reason },
        },
        PolynomialRequest::Gcd { lhs, rhs } => match gcd_univariate(lhs, rhs, rings) {
            Ok(g) => PolynomialResult::Exact { value: PolynomialDomainValue::Polynomial(PolynomialValue { inner: g }) },
            Err(reason) => PolynomialResult::Unevaluated { reason },
        },
        PolynomialRequest::Groebner { generators, limits } => match compute_groebner_basis(generators, rings, limits) {
            Ok(computation) => PolynomialResult::Exact {
                value: PolynomialDomainValue::GroebnerBasis(GroebnerBasisValue::from_computation(computation)),
            },
            Err(reason) => PolynomialResult::Unevaluated { reason },
        },
        PolynomialRequest::Eliminate { generators, limits } => match compute_elimination_basis(generators, rings, limits) {
            Ok(computation) => PolynomialResult::Exact {
                value: PolynomialDomainValue::GroebnerBasis(GroebnerBasisValue::from_computation(computation)),
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
        PolynomialRequest::Eliminate { .. } => "eliminate",
    };
    PolynomialResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "polynomial").detail("operation", op),
    }
}
