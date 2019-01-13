//! 单变量除法 · GCD · Resultant（ℤ / ℚ / 𝔽_p）。

use athena_engine::{
    CoefficientDomain, DivisionPolicy, Integer, MonomialOrder, Number, PolynomialBuilder, PolynomialRequest, PolynomialResult,
    RingTable, SymbolId, execute_polynomial_with_rings, gcd_univariate, resultant_univariate,
};

fn z_x() -> (RingTable, athena_engine::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    (rings, id)
}

fn q_x() -> (RingTable, athena_engine::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    (rings, id)
}

fn f7_x() -> (RingTable, athena_engine::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern_over_prime_field(Integer::from_i64(7), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    (rings, id)
}

fn uni(rings: &RingTable, ring: athena_engine::RingId, terms: &[(i64, u32)]) -> athena_engine::Polynomial {
    let mut b = PolynomialBuilder::new(ring);
    for &(c, d) in terms {
        b.push_term(Number::small_int(c), vec![d]).unwrap();
    }
    b.build(rings).unwrap()
}

#[test]
fn rational_univariate_division() {
    let (rings, ring) = q_x();
    let dividend = uni(&rings, ring, &[(1, 2), (-1, 0)]);
    let divisor = uni(&rings, ring, &[(1, 1), (-1, 0)]);
    let result = execute_polynomial_with_rings(
        PolynomialRequest::Div { dividend, divisor, policy: DivisionPolicy::FieldDivision },
        &rings,
    );
    match result {
        PolynomialResult::Exact { value } => match value {
            athena_engine::PolynomialDomainValue::UnivariateDivision(d) => {
                assert!(d.remainder.inner.terms().is_empty());
                assert_eq!(d.quotient.inner.terms()[0].exponents(), vec![1]);
                assert_eq!(d.quotient.inner.terms()[0].coefficient().to_render_string(), "1");
                assert_eq!(d.quotient.inner.terms()[1].exponents(), vec![0]);
                assert_eq!(d.quotient.inner.terms()[1].coefficient().to_render_string(), "1");
            }
            other => panic!("unexpected value {other:?}"),
        },
        other => panic!("unexpected result {other:?}"),
    }
}

#[test]
fn integer_exact_division_rejects_nonzero_remainder() {
    let (rings, ring) = z_x();
    let dividend = uni(&rings, ring, &[(1, 1), (1, 0)]);
    let divisor = uni(&rings, ring, &[(2, 1), (1, 0)]);
    let err =
        execute_polynomial_with_rings(PolynomialRequest::Div { dividend, divisor, policy: DivisionPolicy::ExactOnly }, &rings);
    assert!(matches!(err, PolynomialResult::Unevaluated { .. }));
}

#[test]
fn integer_gcd_is_monic_primitive() {
    let (rings, ring) = z_x();
    let a = uni(&rings, ring, &[(1, 2), (-1, 0)]);
    let b = uni(&rings, ring, &[(1, 1), (-1, 0)]);
    let g = gcd_univariate(a, b, &rings).unwrap();
    assert_eq!(g.terms().len(), 2);
    assert!(g.terms().iter().any(|t| t.exponents() == vec![1] && t.coefficient().to_render_string() == "1"));
}

#[test]
fn prime_field_gcd() {
    let (rings, ring) = f7_x();
    let a = uni(&rings, ring, &[(3, 1), (1, 0)]);
    let b = uni(&rings, ring, &[(2, 1), (1, 0)]);
    let g = gcd_univariate(a, b, &rings).unwrap();
    assert_eq!(g.terms().len(), 1);
    assert_eq!(g.terms()[0].exponents(), vec![0]);
    assert_eq!(g.terms()[0].coefficient().to_render_string(), "1");
}

#[test]
fn resultant_linear_pair() {
    let (rings, ring) = q_x();
    let a = uni(&rings, ring, &[(1, 1), (2, 0)]);
    let b = uni(&rings, ring, &[(1, 1), (3, 0)]);
    let res = resultant_univariate(a, b, &rings).unwrap();
    assert_eq!(res.to_render_string(), "1");
}

#[test]
fn resultant_symmetry_under_swap() {
    let (rings, ring) = q_x();
    let a = uni(&rings, ring, &[(1, 1), (2, 0)]);
    let b = uni(&rings, ring, &[(1, 1), (3, 0)]);
    let rab = resultant_univariate(a.clone(), b.clone(), &rings).unwrap();
    let rba = resultant_univariate(b, a, &rings).unwrap();
    assert_eq!(rab.to_render_string(), "1");
    assert_eq!(rba.to_render_string(), "-1");
}

#[test]
fn gcd_request_via_execute_polynomial() {
    let (rings, ring) = q_x();
    let a = uni(&rings, ring, &[(1, 2), (1, 0)]);
    let b = uni(&rings, ring, &[(1, 1), (1, 0)]);
    let result = execute_polynomial_with_rings(PolynomialRequest::Gcd { lhs: a, rhs: b }, &rings);
    match result {
        PolynomialResult::Exact { value } => match value {
            athena_engine::PolynomialDomainValue::Polynomial(p) => {
                assert_eq!(p.inner.terms().len(), 1);
            }
            other => panic!("unexpected {other:?}"),
        },
        other => panic!("unexpected {other:?}"),
    }
}
