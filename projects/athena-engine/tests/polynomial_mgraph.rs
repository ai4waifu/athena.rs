//! M-Graph 多项式缓存、admission gate 与 JIT parity 矩阵。

use athena_engine::{
    AdmissionOutcome, AdmissionRejectReason, CoefficientDomain, GroebnerLimits, Guarantee, MonomialOrder, Number,
    PolynomialBuilder, PolynomialCacheOp, PolynomialCacheTier, PolynomialDomainValue, PolynomialRequest, PolynomialResult,
    Scope, Session, SymbolId, admit_polynomial_result, cache_key_for_request, record_polynomial_result,
};

fn z_x_ring(session: &mut Session) -> athena_engine::RingId {
    session
        .rings
        .intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex)
        .unwrap()
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
    assert_eq!(session.mgraph.polynomial.len(), 1);
    assert_eq!(session.mgraph.witnesses.len(), 1);
    assert_eq!(session.mgraph.verified_claims.len(), 1);
    let r2 = session.execute_polynomial_mgraph(req);
    assert_eq!(r1, r2);
    assert_eq!(session.mgraph.polynomial.len(), 1);
}

#[test]
fn cache_key_distinguishes_operations() {
    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let p = PolynomialBuilder::new(ring).build(&session.rings).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![0]).unwrap();
    let q = b.build(&session.rings).unwrap();
    let k_add = cache_key_for_request(&PolynomialRequest::Add { lhs: p.clone(), rhs: q.clone() }, &session.rings).unwrap();
    let k_mul = cache_key_for_request(&PolynomialRequest::Mul { lhs: p, rhs: q }, &session.rings).unwrap();
    assert_ne!(k_add.operation, k_mul.operation);
    assert_eq!(k_add.operation, PolynomialCacheOp::Add);
}

#[test]
fn groebner_complete_admitted_to_claims() {
    let mut session = Session::default();
    let ring = session
        .rings
        .intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex)
        .unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![1]).unwrap();
    b.push_term(Number::small_int(-1), vec![0]).unwrap();
    let g = b.build(&session.rings).unwrap();
    let req = PolynomialRequest::Groebner {
        generators: vec![g],
        limits: GroebnerLimits::default(),
    };
    let r = session.execute_polynomial_mgraph(req.clone());
    assert!(matches!(r, PolynomialResult::Exact { .. }));
    assert_eq!(session.mgraph.polynomial.len(), 1);
    assert_eq!(session.mgraph.witnesses.len(), 1);
    assert_eq!(session.mgraph.verified_claims.len(), 1);
    let key = cache_key_for_request(&req, &session.rings).unwrap();
    let entry = session.mgraph.polynomial.get(&key).unwrap();
    assert!(entry.witness.is_some());
    assert!(entry.witness.as_ref().unwrap().groebner_steps.is_some());
    let vc = &session.mgraph.verified_claims[0];
    assert_eq!(vc.claim.scope, Scope::Unconditional);
    assert_eq!(vc.claim.guarantee, Guarantee::ProvenExact);
    assert!(vc.admissible_for_exact_union());
}

#[test]
fn groebner_partial_cached_but_not_admitted() {
    let mut session = Session::default();
    let ring = session
        .rings
        .intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex)
        .unwrap();
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
        limits: GroebnerLimits {
            max_s_pairs: 0,
            max_basis_size: 128,
        },
    };
    let r = session.execute_polynomial_mgraph(req.clone());
    assert!(matches!(r, PolynomialResult::Exact { .. }));
    assert_eq!(session.mgraph.polynomial.len(), 1);
    assert_eq!(session.mgraph.witnesses.len(), 0);
    assert_eq!(session.mgraph.verified_claims.len(), 0);
    let key = cache_key_for_request(&req, &session.rings).unwrap();
    let entry = session.mgraph.polynomial.get(&key).unwrap();
    assert!(entry.witness.is_none());
    match admit_polynomial_result(&key, &entry.result) {
        AdmissionOutcome::Rejected {
            reason: AdmissionRejectReason::GroebnerIncomplete,
            guarantee: Guarantee::Partial,
        } => {}
        other => panic!("expected GroebnerIncomplete rejection, got {other:?}"),
    }
}

#[test]
fn placeholder_exact_result_not_admitted() {
    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let key = cache_key_for_request(
        &PolynomialRequest::Normalize {
            polynomial: PolynomialBuilder::new(ring).build(&session.rings).unwrap(),
        },
        &session.rings,
    )
    .unwrap();
    let result = PolynomialResult::Exact {
        value: PolynomialDomainValue::Placeholder,
    };
    record_polynomial_result(key.clone(), result, &mut session.mgraph).unwrap();
    assert_eq!(session.mgraph.polynomial.len(), 1);
    assert_eq!(session.mgraph.polynomial.partial_len(), 1);
    assert_eq!(session.mgraph.witnesses.len(), 0);
    assert_eq!(session.mgraph.verified_claims.len(), 0);
    match admit_polynomial_result(&key, &session.mgraph.polynomial.get_partial(&key).unwrap().result) {
        AdmissionOutcome::Rejected {
            reason: AdmissionRejectReason::Placeholder,
            ..
        } => {}
        other => panic!("expected Placeholder rejection, got {other:?}"),
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
    assert!(athena_engine::parity_diagnostic(&outcome).is_none());
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
    let (prod, outcome) = athena_engine::mul_with_jit_parity(lhs, rhs, &session.rings).unwrap();
    assert_eq!(prod.terms.len(), 1);
    assert_eq!(outcome, athena_engine::JitParityOutcome::JitUnavailable);
    assert!(athena_engine::parity_diagnostic(&outcome).is_none());
}

#[cfg(feature = "jit")]
#[test]
fn jit_parity_eager_semantics_unchanged_when_backend_unavailable() {
    use athena_engine::{mul_polynomial, parity_diagnostic, JitParityOutcome, mul_with_jit_parity};

    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(3), vec![2]).unwrap();
    let lhs = b1.build(&session.rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(5), vec![1]).unwrap();
    let rhs = b2.build(&session.rings).unwrap();
    let eager = mul_polynomial(lhs.clone(), rhs.clone(), &session.rings).unwrap();
    let (via_parity, outcome) = mul_with_jit_parity(lhs, rhs, &session.rings).unwrap();
    assert_eq!(eager, via_parity);
    assert_eq!(outcome, JitParityOutcome::JitUnavailable);
    assert!(parity_diagnostic(&outcome).is_none());
}
