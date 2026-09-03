//! 多项式因式分解骨架合同。

use athena_engine::{
    CoefficientDomain, MonomialOrder, Number, PolynomialBuilder, PolynomialDomainValue, PolynomialFactorLimits,
    PolynomialFactorStatus, PolynomialFactorizationCompleteness, PolynomialRequest, PolynomialResult, RingTable, SymbolId,
    execute_polynomial_with_rings, factor_univariate,
};

fn q_x() -> (RingTable, athena_engine::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
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
fn constant_factors_complete() {
    let (rings, ring) = q_x();
    let p = uni(&rings, ring, &[(5, 0)]);
    let f = factor_univariate(p, &rings, PolynomialFactorLimits::default()).unwrap();
    assert_eq!(f.completeness(), PolynomialFactorizationCompleteness::Complete);
    assert!(f.factors.is_empty());
    assert_eq!(f.unit.to_render_string(), "5");
}

#[test]
fn linear_factors_proven_irreducible() {
    let (rings, ring) = q_x();
    let p = uni(&rings, ring, &[(1, 1), (1, 0)]);
    let f = factor_univariate(p, &rings, PolynomialFactorLimits::default()).unwrap();
    assert_eq!(f.completeness(), PolynomialFactorizationCompleteness::Complete);
    assert_eq!(f.factors.len(), 1);
    assert_eq!(f.factors[0].status, PolynomialFactorStatus::ProvenIrreducible);
    assert_eq!(f.factors[0].exponent, 1);
}

#[test]
fn quadratic_irreducible_over_rationals_is_complete() {
    let (rings, ring) = q_x();
    let p = uni(&rings, ring, &[(1, 2), (1, 0)]);
    let f = factor_univariate(p, &rings, PolynomialFactorLimits::default()).unwrap();
    assert_eq!(f.completeness(), PolynomialFactorizationCompleteness::Complete);
    assert_eq!(f.factors.len(), 1);
    assert_eq!(f.factors[0].status, PolynomialFactorStatus::ProvenIrreducible);
    assert!(f.cofactor.terms().is_empty());
    assert!(f.is_exact_witness());
}

#[test]
fn quadratic_difference_of_squares_factors_complete() {
    let (rings, ring) = q_x();
    let p = uni(&rings, ring, &[(1, 2), (-1, 0)]);
    let f = factor_univariate(p, &rings, PolynomialFactorLimits::default()).unwrap();
    assert_eq!(f.completeness(), PolynomialFactorizationCompleteness::Complete);
    assert_eq!(f.factors.len(), 2);
    assert!(f.factors.iter().all(|c| c.status == PolynomialFactorStatus::ProvenIrreducible));
}

#[test]
fn degree_limit_resource_limited() {
    let (rings, ring) = q_x();
    let p = uni(&rings, ring, &[(1, 3)]);
    let limits = PolynomialFactorLimits { max_degree: 2, max_steps: 10 };
    let f = factor_univariate(p, &rings, limits).unwrap();
    assert_eq!(f.completeness(), PolynomialFactorizationCompleteness::ResourceLimited);
    assert!(f.input_rejected);
}

#[test]
fn factor_request_via_execute_polynomial() {
    let (rings, ring) = q_x();
    let p = uni(&rings, ring, &[(2, 1), (3, 0)]);
    let result = execute_polynomial_with_rings(
        PolynomialRequest::Factor { polynomial: p, limits: PolynomialFactorLimits::default() },
        &rings,
    );
    match result {
        PolynomialResult::Exact { value: PolynomialDomainValue::Factorization(f) } => {
            assert_eq!(f.completeness(), PolynomialFactorizationCompleteness::Complete);
        }
        other => panic!("unexpected {other:?}"),
    }
}
