//! M-Graph 多项式缓存、admission gate 与 JIT parity 矩阵。

use athena_engine::{
    AdmissionOutcome, AdmissionRejectReason, Claim, CoefficientDomain, Evidence, EvidenceVerifier, GroebnerLimits, Guarantee,
    MonomialOrder, Number, PolynomialBuilder, PolynomialDomainValue, PolynomialRequest,
    PolynomialResult, Scope, Session, SymbolId, VerificationPolicy, admit_polynomial_result, cache_key_for_request,
    proposition_from_cache_key, record_polynomial_result,
};

fn z_x_ring(session: &mut Session) -> athena_engine::RingId {
    session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap()
}

#[test]
fn mgraph_polynomial_cache_hit() {
    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![1]).unwrap();
    let a = b.build(&session.rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(2), vec![0]).unwrap();
    let c = b2.build(&session.rings).unwrap();
    let req = PolynomialRequest::Add { lhs: a.clone(), rhs: c.clone() };
    let r1 = session.execute_polynomial_mgraph(req.clone());
    assert!(matches!(r1, PolynomialResult::Exact { .. }));
    assert_eq!(session.mgraph.operational.result_cache.polynomial.len(), 1);
    assert_eq!(session.mgraph.semantic.derived.rewrite_witnesses.len(), 1);
    assert_eq!(session.mgraph.semantic.fact_log.len(), 1);
    let r2 = session.execute_polynomial_mgraph(req);
    assert_eq!(r1, r2);
}

#[test]
fn groebner_complete_admitted_to_claims() {
    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![1]).unwrap();
    b.push_term(Number::small_int(-1), vec![0]).unwrap();
    let g = b.build(&session.rings).unwrap();
    let req = PolynomialRequest::Groebner { generators: vec![g], limits: GroebnerLimits::default() };
    session.execute_polynomial_mgraph(req.clone());
    assert_eq!(session.mgraph.semantic.fact_log.len(), 1);
    let key = cache_key_for_request(&req, &session.rings).unwrap();
    let vc = session.mgraph.semantic.fact_log.get(athena_engine::FactId(0)).unwrap();
    assert_eq!(vc.claim.guarantee, Guarantee::ProvenExact);
    assert!(session.mgraph.operational.result_cache.polynomial.get(&key).unwrap().witness.is_some());
}

#[test]
fn groebner_partial_cached_but_not_admitted() {
    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(1), vec![1, 0]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0, 1]).unwrap();
    let g1 = b1.build(&session.rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(1), vec![0, 1]).unwrap();
    b2.push_term(Number::small_int(-1), vec![0, 0]).unwrap();
    let g2 = b2.build(&session.rings).unwrap();
    let req = PolynomialRequest::Groebner {
        generators: vec![g1, g2],
        limits: GroebnerLimits { max_s_pairs: 0, max_basis_size: 128 },
    };
    session.execute_polynomial_mgraph(req.clone());
    assert_eq!(session.mgraph.semantic.fact_log.len(), 0);
    assert_eq!(session.mgraph.operational.result_cache.polynomial.partial_len(), 1);
    let key = cache_key_for_request(&req, &session.rings).unwrap();
    match admit_polynomial_result(&key, &session.mgraph.operational.result_cache.polynomial.get_partial(&key).unwrap().result) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::GroebnerIncomplete, guarantee: Guarantee::Partial } => {}
        other => panic!("expected GroebnerIncomplete, got {other:?}"),
    }
}

#[test]
fn placeholder_exact_result_not_admitted() {
    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let key = cache_key_for_request(
        &PolynomialRequest::Normalize { polynomial: PolynomialBuilder::new(ring).build(&session.rings).unwrap() },
        &session.rings,
    )
    .unwrap();
    record_polynomial_result(
        key.clone(),
        PolynomialResult::Exact { value: PolynomialDomainValue::Placeholder },
        &mut session.mgraph,
    )
    .unwrap();
    assert_eq!(session.mgraph.semantic.fact_log.len(), 0);
    match admit_polynomial_result(&key, &session.mgraph.operational.result_cache.polynomial.get_partial(&key).unwrap().result) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::Placeholder, .. } => {}
        other => panic!("expected Placeholder, got {other:?}"),
    }
}

#[test]
fn probable_claim_blocked_by_verifier() {
    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let key = cache_key_for_request(
        &PolynomialRequest::Add {
            lhs: PolynomialBuilder::new(ring).build(&session.rings).unwrap(),
            rhs: PolynomialBuilder::new(ring).build(&session.rings).unwrap(),
        },
        &session.rings,
    )
    .unwrap();
    let claim = Claim {
        proposition: proposition_from_cache_key(&key),
        scope: Scope::Unconditional,
        guarantee: Guarantee::Probable,
        evidence: Evidence::TrustedKernel { solver: athena_engine::POLYNOMIAL_SOLVER_ID, summary: "probable".into() },
    };
    match EvidenceVerifier::verify(&claim, &VerificationPolicy::default()) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::ProbableResult, guarantee: Guarantee::Probable } => {}
        other => panic!("expected ProbableResult, got {other:?}"),
    }
}

#[cfg(not(feature = "jit"))]
#[test]
fn jit_parity_without_jit_feature_is_eager_only() {
    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(1), vec![1]).unwrap();
    let lhs = b1.build(&session.rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(2), vec![1]).unwrap();
    let rhs = b2.build(&session.rings).unwrap();
    let (prod, outcome) = athena_engine::mul_with_jit_parity(lhs, rhs, &session.rings).unwrap();
    assert_eq!(prod.terms.len(), 1);
    assert_eq!(outcome, athena_engine::JitParityOutcome::EagerOnly);
}

#[cfg(feature = "jit")]
#[test]
fn jit_parity_with_jit_feature_backend_unavailable() {
    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(1), vec![1]).unwrap();
    let lhs = b1.build(&session.rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(2), vec![1]).unwrap();
    let rhs = b2.build(&session.rings).unwrap();
    let (_, outcome) = athena_engine::mul_with_jit_parity(lhs, rhs, &session.rings).unwrap();
    assert_eq!(outcome, athena_engine::JitParityOutcome::JitUnavailable);
}
