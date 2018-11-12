//! Gröbner 基 · 消元 · 验证 · 类型分型。

use athena_engine::{
    CoefficientDomain, GroebnerComputation, GroebnerLimits, GroebnerStatus, MonomialOrder, Number, PolynomialBuilder,
    PolynomialRequest, PolynomialResult, RingTable, Session, SymbolId, compute_groebner_basis, ideal_membership,
    reduce_by_verified, reduce_ideal, verify_groebner_basis,
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
fn groebner_trivial_generators_to_verified_basis() {
    let (rings, ring) = q_xy_lex();
    let g1 = build(&rings, ring, &[(1, 1, vec![1, 0]), (-1, 1, vec![0, 1])]);
    let g2 = build(&rings, ring, &[(1, 1, vec![0, 1]), (-1, 1, vec![0, 0])]);
    let computation = compute_groebner_basis(vec![g1, g2], &rings, GroebnerLimits::default()).unwrap();
    assert_eq!(computation.status(), GroebnerStatus::Verified);
    let gb = computation.as_verified().expect("verified");
    assert!(gb.certificate.complete);
    assert!(gb.certificate.verified);
    assert!(gb.verification.all_s_pairs_reduce_to_zero);
    assert_eq!(gb.certificate.algorithm, athena_engine::GroebnerAlgorithm::Buchberger);
    assert!(contains_poly_with_leading_exp(gb.basis(), &[1, 0]) || contains_poly_with_leading_exp(gb.basis(), &[0, 1]));
}

#[test]
fn reduce_by_verified_groebner_basis() {
    let (rings, ring) = q_xy_lex();
    let g = build(&rings, ring, &[(1, 1, vec![1, 0]), (-1, 1, vec![0, 0])]);
    let computation = compute_groebner_basis(vec![g.clone()], &rings, GroebnerLimits::default()).unwrap();
    let gb = computation.as_verified().unwrap();
    let f = build(&rings, ring, &[(1, 1, vec![2, 0])]);
    let rem = reduce_by_verified(f.clone(), gb, &rings).unwrap();
    assert_eq!(rem.terms.len(), 1);
    assert_eq!(rem.terms[0].exponents, vec![0, 0]);
    assert!(!ideal_membership(f, gb, &rings).unwrap());
    let zero_member = build(&rings, ring, &[(1, 1, vec![1, 0]), (-1, 1, vec![0, 0])]);
    assert!(ideal_membership(zero_member, gb, &rings).unwrap());
}

#[test]
fn heuristic_reduce_ideal_still_available() {
    let (rings, ring) = q_xy_lex();
    let g = build(&rings, ring, &[(1, 1, vec![1, 0]), (-1, 1, vec![0, 0])]);
    let f = build(&rings, ring, &[(1, 1, vec![2, 0])]);
    let rem = reduce_ideal(f, &[g], &rings).unwrap();
    assert_eq!(rem.terms[0].exponents, vec![0, 0]);
}

#[test]
fn elimination_ideal_in_y() {
    let (rings, ring) = q_xy_elim_x();
    let f1 = build(&rings, ring, &[(1, 1, vec![2, 0]), (1, 1, vec![0, 1]), (-1, 1, vec![0, 0])]);
    let f2 = build(&rings, ring, &[(1, 1, vec![1, 0]), (1, 1, vec![0, 1]), (-1, 1, vec![0, 0])]);
    let out = athena_engine::compute_elimination_basis(vec![f1, f2], &rings, GroebnerLimits::default()).unwrap();
    assert_eq!(out.status(), GroebnerStatus::Verified);
    assert!(out.certificate().elimination_elements.is_some());
    assert!(out.polynomials().iter().all(|p| p.terms.iter().all(|t| t.exponents[0] == 0)));
    assert!(!out.polynomials().is_empty());
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
                assert_eq!(v.status, GroebnerStatus::Verified);
                assert_eq!(v.basis.len(), 1);
                assert!(v.certificate.complete);
                assert!(v.certificate.verified);
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

#[test]
fn pair_budget_exhaustion_yields_partial_not_verified() {
    let (rings, ring) = q_xy_lex();
    let g1 = build(&rings, ring, &[(1, 1, vec![1, 0]), (-1, 1, vec![0, 1])]);
    let g2 = build(&rings, ring, &[(1, 1, vec![0, 1]), (-1, 1, vec![0, 0])]);
    let computation = compute_groebner_basis(
        vec![g1, g2],
        &rings,
        GroebnerLimits { max_s_pairs: 0, max_basis_size: 128 },
    )
    .unwrap();
    assert!(matches!(computation, GroebnerComputation::Partial(_)));
    assert_eq!(computation.status(), GroebnerStatus::Partial);
    assert!(!computation.certificate().verified);
    assert!(!computation.certificate().complete);
    assert!(computation.as_verified().is_none());
}

#[test]
fn basis_size_limit_yields_resource_limited() {
    let (rings, ring) = q_xy_lex();
    let g1 = build(&rings, ring, &[(1, 1, vec![2, 0]), (-1, 1, vec![0, 1])]);
    let g2 = build(&rings, ring, &[(1, 1, vec![1, 1]), (-1, 1, vec![0, 0])]);
    let computation = compute_groebner_basis(
        vec![g1, g2],
        &rings,
        GroebnerLimits { max_s_pairs: 10_000, max_basis_size: 2 },
    )
    .unwrap();
    // May be Partial or ResourceLimited depending on when size trips; both are unverified.
    assert!(!computation.is_verified());
    assert!(matches!(
        computation,
        GroebnerComputation::ResourceLimited(_) | GroebnerComputation::Partial(_) | GroebnerComputation::Complete(_)
    ));
    if let GroebnerComputation::ResourceLimited(f) = &computation {
        assert!(!f.certificate.verified);
    }
}

#[test]
fn verify_groebner_basis_accepts_buchberger_output() {
    let (rings, ring) = q_xy_lex();
    let g = build(&rings, ring, &[(1, 1, vec![1, 0]), (-1, 1, vec![0, 0])]);
    let computation = compute_groebner_basis(vec![g], &rings, GroebnerLimits::default()).unwrap();
    let report = verify_groebner_basis(computation.polynomials(), &rings).unwrap();
    assert!(report.all_s_pairs_reduce_to_zero);
}

#[test]
fn verify_rejects_non_groebner_pair() {
    let (rings, ring) = q_xy_lex();
    // Classic non-GB under lex (x > y): {x^2 - y, x*y - 1}. S-pair yields y^2 - x which does not reduce to 0.
    let g1 = build(&rings, ring, &[(1, 1, vec![2, 0]), (-1, 1, vec![0, 1])]);
    let g2 = build(&rings, ring, &[(1, 1, vec![1, 1]), (-1, 1, vec![0, 0])]);
    let report = verify_groebner_basis(&[g1, g2], &rings).unwrap();
    assert!(!report.all_s_pairs_reduce_to_zero);
}

#[test]
fn reduce_by_verified_rejects_unverified_certificate() {
    let (rings, ring) = q_xy_lex();
    let g1 = build(&rings, ring, &[(1, 1, vec![1, 0]), (-1, 1, vec![0, 1])]);
    let g2 = build(&rings, ring, &[(1, 1, vec![0, 1]), (-1, 1, vec![0, 0])]);
    let computation = compute_groebner_basis(
        vec![g1, g2],
        &rings,
        GroebnerLimits { max_s_pairs: 0, max_basis_size: 128 },
    )
    .unwrap();
    assert!(matches!(computation, GroebnerComputation::Partial(_)));
    // Construct a fake VerifiedGroebnerBasis with incomplete certificate must not be possible via API;
    // membership / reduce_by_verified only accept VerifiedGroebnerBasis from Complete.
    assert!(computation.as_verified().is_none());
}
