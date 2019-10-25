//! ℤ / ℚ / 𝔽_p 精确多项式内核：加减乘与环律。

use std::str::FromStr;

use athena_engine::{
    domains::polynomial::{
        CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialRequest, PolynomialResult, RingTable, add_polynomial,
        execute_polynomial, mul_polynomial, sub_polynomial,
    },
    runtime::Session,
};
use athena_numeric::{Integer, Number};
use athena_types::SymbolId;

fn z_x_ring() -> (RingTable, athena_engine::domains::polynomial::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    (rings, id)
}

fn q_x_ring() -> (RingTable, athena_engine::domains::polynomial::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    (rings, id)
}

fn f5_x_ring() -> (RingTable, athena_engine::domains::polynomial::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    (rings, id)
}

fn build_univariate(
    rings: &RingTable,
    ring: athena_engine::domains::polynomial::RingId,
    terms: &[(i64, u32)],
) -> athena_engine::domains::polynomial::Polynomial {
    let mut b = PolynomialBuilder::new(ring);
    for &(c, d) in terms {
        b.push_term(Number::small_int(c), vec![d]).unwrap();
    }
    b.build(rings).unwrap()
}

fn coeff_at_degree(poly: &athena_engine::domains::polynomial::Polynomial, d: u32) -> Option<String> {
    poly.terms().iter().find(|t| t.exponents() == vec![d]).map(|t| t.coefficient().to_render_string())
}

#[test]
fn integer_add_cancels_to_zero() {
    let (rings, ring) = z_x_ring();
    let a = build_univariate(&rings, ring, &[(1, 1), (1, 0)]);
    let b = build_univariate(&rings, ring, &[(-1, 1), (-1, 0)]);
    let sum = add_polynomial(a, b, &rings).unwrap();
    assert!(sum.terms().is_empty());
}

#[test]
fn integer_mul_quadratic() {
    let (rings, ring) = z_x_ring();
    let a = build_univariate(&rings, ring, &[(1, 1), (2, 0)]);
    let b = build_univariate(&rings, ring, &[(1, 1), (3, 0)]);
    let prod = mul_polynomial(a, b, &rings).unwrap();
    assert_eq!(coeff_at_degree(&prod, 0).as_deref(), Some("6"));
    assert_eq!(coeff_at_degree(&prod, 1).as_deref(), Some("5"));
    assert_eq!(coeff_at_degree(&prod, 2).as_deref(), Some("1"));
}

#[test]
fn rational_mul_mixed_coefficients() {
    let (rings, ring) = q_x_ring();
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::rational_i64(1, 2).unwrap(), vec![1]).unwrap();
    b1.push_term(Number::small_int(1), vec![0]).unwrap();
    let p1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(2), vec![1]).unwrap();
    let p2 = b2.build(&rings).unwrap();
    let prod = mul_polynomial(p1, p2, &rings).unwrap();
    assert_eq!(coeff_at_degree(&prod, 1).as_deref(), Some("2"));
    assert_eq!(coeff_at_degree(&prod, 2).as_deref(), Some("1"));
}

#[test]
fn prime_field_mul_reduces_mod_p() {
    let (rings, ring) = f5_x_ring();
    let a = build_univariate(&rings, ring, &[(2, 1), (1, 0)]);
    let b = build_univariate(&rings, ring, &[(3, 1), (1, 0)]);
    let prod = mul_polynomial(a, b, &rings).unwrap();
    assert_eq!(coeff_at_degree(&prod, 0).as_deref(), Some("1"));
    assert!(coeff_at_degree(&prod, 1).is_none());
    assert_eq!(coeff_at_degree(&prod, 2).as_deref(), Some("1"));
}

#[test]
fn sub_matches_add_neg() {
    let (rings, ring) = z_x_ring();
    let a = build_univariate(&rings, ring, &[(3, 2), (1, 0)]);
    let b = build_univariate(&rings, ring, &[(1, 1)]);
    let diff = sub_polynomial(a.clone(), b.clone(), &rings).unwrap();
    let via_add = add_polynomial(a, build_univariate(&rings, ring, &[(-1, 1)]), &rings).unwrap();
    assert_eq!(diff, via_add);
}

#[test]
fn ring_mismatch_rejected() {
    let mut rings = RingTable::new();
    let ring_z = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let ring_q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let a = build_univariate(&rings, ring_z, &[(1, 0)]);
    let b = PolynomialBuilder::new(ring_q).build(&rings).unwrap();
    let err = add_polynomial(a, b, &rings).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_DOMAIN_MISMATCH");
}

#[test]
fn large_integer_coefficients_mul() {
    let (rings, ring) = z_x_ring();
    let big = Integer::from_str("99999999999999999999").unwrap();
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::integer(big), vec![1]).unwrap();
    let p1 = b1.build(&rings).unwrap();
    let p2 = build_univariate(&rings, ring, &[(2, 1)]);
    let prod = mul_polynomial(p1, p2, &rings).unwrap();
    let c1 = prod.terms().iter().find(|t| t.exponents() == vec![2]).unwrap();
    assert_eq!(c1.coefficient().to_render_string(), "199999999999999999998");
}

#[test]
fn distributive_law_smoke() {
    let (rings, ring) = z_x_ring();
    let a = build_univariate(&rings, ring, &[(2, 1)]);
    let b = build_univariate(&rings, ring, &[(1, 1), (1, 0)]);
    let c = build_univariate(&rings, ring, &[(3, 0)]);
    let left = mul_polynomial(a.clone(), add_polynomial(b.clone(), c.clone(), &rings).unwrap(), &rings).unwrap();
    let right = add_polynomial(mul_polynomial(a.clone(), b, &rings).unwrap(), mul_polynomial(a, c, &rings).unwrap(), &rings).unwrap();
    assert_eq!(left, right);
}

#[test]
fn session_mul_via_execute_polynomial() {
    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let a = build_univariate(&session.rings, ring, &[(1, 1)]);
    let b = build_univariate(&session.rings, ring, &[(1, 1)]);
    let lhs = session.polynomial_objects.intern(a, &session.rings);
    let rhs = session.polynomial_objects.intern(b, &session.rings);
    let out = session.execute_polynomial(PolynomialRequest::Mul { lhs, rhs });
    match out {
        PolynomialResult::Exact { value } => {
            let poly = match value {
                athena_engine::domains::polynomial::PolynomialDomainValue::Polynomial(v) => v.inner,
                _ => panic!("expected polynomial"),
            };
            assert_eq!(coeff_at_degree(&poly, 2).as_deref(), Some("1"));
        }
        other => panic!("expected Exact, got {other:?}"),
    }
}

#[test]
fn stateless_execute_polynomial_mul_unevaluated() {
    let out = execute_polynomial(PolynomialRequest::Mul {
        lhs: athena_engine::domains::polynomial::PolynomialRef(0),
        rhs: athena_engine::domains::polynomial::PolynomialRef(1),
    });
    match out {
        PolynomialResult::Unevaluated { reason } => {
            assert_eq!(reason.code.as_str(), "ATHENA_UNSUPPORTED_OPERATION");
        }
        other => panic!("expected Unevaluated, got {other:?}"),
    }
}
