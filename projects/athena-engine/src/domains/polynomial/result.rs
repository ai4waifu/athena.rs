//! 多项式域分派（经 [`PolynomialObjectStore`] 解析 [`PolynomialRef`]）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    canonical::canonicalize_polynomial,
    factor::factor_univariate,
    groebner::{compute_elimination_basis, compute_groebner_basis},
    object_ref::PolynomialObjectStore,
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

/// 无 Session 仓时不可执行（须 [`execute_polynomial_with_rings`]）。
pub fn execute_polynomial(request: PolynomialRequest) -> PolynomialResult {
    PolynomialResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", operation_name(&request))
            .detail("hint", "use execute_polynomial_with_rings with PolynomialObjectStore"),
    }
}

/// 在已注册环表与 DomainObject 仓中执行多项式域请求。
pub fn execute_polynomial_with_rings(request: PolynomialRequest, rings: &RingTable, store: &PolynomialObjectStore) -> PolynomialResult {
    match request {
        PolynomialRequest::Normalize { polynomial } => {
            let polynomial = match store.resolve_owning(polynomial) {
                Ok(p) => p,
                Err(reason) => return PolynomialResult::Unevaluated { reason },
            };
            match canonicalize_polynomial(polynomial, rings) {
                Ok(normalized) => PolynomialResult::Exact { value: PolynomialDomainValue::Polynomial(PolynomialValue { inner: normalized }) },
                Err(reason) => PolynomialResult::Unevaluated { reason },
            }
        }
        PolynomialRequest::Add { lhs, rhs } => {
            let (lhs, rhs) = match (store.resolve_owning(lhs), store.resolve_owning(rhs)) {
                (Ok(l), Ok(r)) => (l, r),
                (Err(reason), _) | (_, Err(reason)) => return PolynomialResult::Unevaluated { reason },
            };
            match add_polynomial(lhs, rhs, rings) {
                Ok(sum) => PolynomialResult::Exact { value: PolynomialDomainValue::Polynomial(PolynomialValue { inner: sum }) },
                Err(reason) => PolynomialResult::Unevaluated { reason },
            }
        }
        PolynomialRequest::Mul { lhs, rhs } => {
            let (lhs, rhs) = match (store.resolve_owning(lhs), store.resolve_owning(rhs)) {
                (Ok(l), Ok(r)) => (l, r),
                (Err(reason), _) | (_, Err(reason)) => return PolynomialResult::Unevaluated { reason },
            };
            match mul_polynomial(lhs, rhs, rings) {
                Ok(product) => PolynomialResult::Exact { value: PolynomialDomainValue::Polynomial(PolynomialValue { inner: product }) },
                Err(reason) => PolynomialResult::Unevaluated { reason },
            }
        }
        PolynomialRequest::Div { dividend, divisor, policy } => {
            let (dividend, divisor) = match (store.resolve_owning(dividend), store.resolve_owning(divisor)) {
                (Ok(l), Ok(r)) => (l, r),
                (Err(reason), _) | (_, Err(reason)) => return PolynomialResult::Unevaluated { reason },
            };
            match div_univariate(dividend, divisor, policy, rings) {
                Ok(division) => PolynomialResult::Exact {
                    value: PolynomialDomainValue::UnivariateDivision(UnivariateDivisionValue {
                        quotient: PolynomialValue { inner: division.quotient },
                        remainder: PolynomialValue { inner: division.remainder },
                    }),
                },
                Err(reason) => PolynomialResult::Unevaluated { reason },
            }
        }
        PolynomialRequest::Gcd { lhs, rhs } => {
            let (lhs, rhs) = match (store.resolve_owning(lhs), store.resolve_owning(rhs)) {
                (Ok(l), Ok(r)) => (l, r),
                (Err(reason), _) | (_, Err(reason)) => return PolynomialResult::Unevaluated { reason },
            };
            match gcd_univariate(lhs, rhs, rings) {
                Ok(g) => PolynomialResult::Exact { value: PolynomialDomainValue::Polynomial(PolynomialValue { inner: g }) },
                Err(reason) => PolynomialResult::Unevaluated { reason },
            }
        }
        PolynomialRequest::Factor { polynomial, limits } => {
            let polynomial = match store.resolve_owning(polynomial) {
                Ok(p) => p,
                Err(reason) => return PolynomialResult::Unevaluated { reason },
            };
            match factor_univariate(polynomial, rings, limits) {
                Ok(f) => PolynomialResult::Exact { value: PolynomialDomainValue::Factorization(f) },
                Err(reason) => PolynomialResult::Unevaluated { reason },
            }
        }
        PolynomialRequest::Groebner { generators, limits } => {
            let generators = match resolve_generators(store, &generators) {
                Ok(g) => g,
                Err(reason) => return PolynomialResult::Unevaluated { reason },
            };
            match compute_groebner_basis(generators, rings, limits) {
                Ok(computation) => {
                    PolynomialResult::Exact { value: PolynomialDomainValue::GroebnerBasis(GroebnerBasisValue::from_computation(computation)) }
                }
                Err(reason) => PolynomialResult::Unevaluated { reason },
            }
        }
        PolynomialRequest::Eliminate { generators, limits } => {
            let generators = match resolve_generators(store, &generators) {
                Ok(g) => g,
                Err(reason) => return PolynomialResult::Unevaluated { reason },
            };
            match compute_elimination_basis(generators, rings, limits) {
                Ok(computation) => {
                    PolynomialResult::Exact { value: PolynomialDomainValue::GroebnerBasis(GroebnerBasisValue::from_computation(computation)) }
                }
                Err(reason) => PolynomialResult::Unevaluated { reason },
            }
        }
    }
}

fn resolve_generators(
    store: &PolynomialObjectStore,
    generators: &[super::object_ref::PolynomialRef],
) -> athena_types::Result<Vec<super::object::Polynomial>> {
    generators.iter().map(|r| store.resolve_owning(*r)).collect()
}

fn operation_name(request: &PolynomialRequest) -> &'static str {
    match request {
        PolynomialRequest::Normalize { .. } => "normalize",
        PolynomialRequest::Add { .. } => "add",
        PolynomialRequest::Mul { .. } => "mul",
        PolynomialRequest::Div { .. } => "div",
        PolynomialRequest::Gcd { .. } => "gcd",
        PolynomialRequest::Factor { .. } => "factor",
        PolynomialRequest::Groebner { .. } => "groebner",
        PolynomialRequest::Eliminate { .. } => "eliminate",
    }
}
