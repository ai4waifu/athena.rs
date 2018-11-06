//! Gröbner 基 · 消元 · 证书。

use athena_engine::{
    CoefficientDomain, GroebnerLimits, MonomialOrder, Number, PolynomialBuilder, PolynomialRequest, PolynomialResult,
    RingTable, Session, SymbolId, compute_groebner_basis, reduce_ideal,
};

fn q_xy_lex() -> (RingTable, athena_engine::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    (rings, id)
}

fn q_xy_elim_x() -> (RingTable, athena_engine::RingId) {
    let mut rings = RingTable::new();
    let id = rings
        .intern(
            CoefficientDomain::Rational,
            vec![SymbolId(0), SymbolId(1)],
            MonomialOrder::Elimination { eliminate: 1, rest: Box::new(MonomialOrder::Lex) },
        )
        .unwrap();
    (rings, id)
}

fn build(rings: &RingTable, ring: athena_engine::RingId, terms: &[(i64, i64, Vec<u32>)]) -> athena_engine::Polynomial {
    let mut b = PolynomialBuilder::new(ring);
    for &(num, den, ref exp) in terms {
        let c = if den == 1 { Number::small_int(num) } else { Number::rational_i64(num, den).unwrap() };
        b.push_term(c, exp.clone()).unwrap();
    }
    b.build(rings).unwrap()
}

fn contains_poly_with_leading_exp(gb: &[athena_engine::Polynomial], exp: &[u32]) -> bool {
    gb.iter().any(|p| p.terms.first().is_some_and(|t| t.exponents == exp))
}

#[test]
fn groebner_trivial_generators_to_basis() {
    let (rings, ring) = q_xy_lex();
    let g1 = build(&rings, ring, &[(1, 1, vec![1, 0]), (-1, 1, vec![0, 1])]);
    let g2 = build(&rings, ring, &[(1, 1, vec![0, 1]), (-1, 1, vec![0, 0])]);
    let gb = compute_groebner_basis(vec![g1, g2], &rings, GroebnerLimits::default()).unwrap();
    assert!(gb.certificate.complete);
    assert_eq!(gb.certificate.algorithm, athena_engine::GroebnerAlgorithm::Buchberger);
    assert!(contains_poly_with_leading_exp(&gb.basis, &[1, 0]) || contains_poly_with_leading_exp(&gb.basis, &[0, 1]));
}

#[test]
fn reduce_by_groebner_basis() {
    let (rings, ring) = q_xy_lex();
    let g = build(&rings, ring, &[(1, 1, vec![1, 0]), (-1, 1, vec![0, 0])]);
    let gb = compute_groebner_basis(vec![g.clone()], &rings, GroebnerLimits::default()).unwrap();
    let f = build(&rings, ring, &[(1, 1, vec![2, 0])]);
    let rem = reduce_ideal(f, &gb.basis, &rings).unwrap();
    assert_eq!(rem.terms.len(), 1);
    assert_eq!(rem.terms[0].exponents, vec![0, 0]);
}

#[test]
fn elimination_ideal_in_y() {
    let (rings, ring) = q_xy_elim_x();
    let f1 = build(&rings, ring, &[(1, 1, vec![2, 0]), (1, 1, vec![0, 1]), (-1, 1, vec![0, 0])]);
    let f2 = build(&rings, ring, &[(1, 1, vec![1, 0]), (1, 1, vec![0, 1]), (-1, 1, vec![0, 0])]);
    let out = athena_engine::compute_elimination_basis(vec![f1, f2], &rings, GroebnerLimits::default()).unwrap();
    assert!(out.certificate.elimination_elements.is_some());
    assert!(out.basis.iter().all(|p| p.terms.iter().all(|t| t.exponents[0] == 0)));
    assert!(!out.basis.is_empty());
}

#[test]
fn integer_ring_groebner_rejected() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let g = build(&rings, ring, &[(1, 1, vec![1])]);
    let err = compute_groebner_basis(vec![g], &rings, GroebnerLimits::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_POLYNOMIAL_NON_FIELD_DIVISION");
}

#[test]
fn session_groebner_via_execute_polynomial() {
    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let g = build(&session.rings, ring, &[(1, 1, vec![1]), (-1, 1, vec![0])]);
    let out =
        session.execute_polynomial(PolynomialRequest::Groebner { generators: vec![g], limits: GroebnerLimits::default() });
    match out {
        PolynomialResult::Exact { value } => match value {
            athena_engine::PolynomialDomainValue::GroebnerBasis(v) => {
                assert_eq!(v.basis.len(), 1);
                assert!(v.certificate.complete);
            }
            _ => panic!("expected GroebnerBasis"),
        },
        other => panic!("expected Exact, got {other:?}"),
    }
}

#[test]
fn eliminate_requires_elimination_order() {
    let (rings, ring) = q_xy_lex();
    let g = build(&rings, ring, &[(1, 1, vec![1, 0])]);
    let err = athena_engine::compute_elimination_basis(vec![g], &rings, GroebnerLimits::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_POLYNOMIAL_ORDER_INVALID");
}
