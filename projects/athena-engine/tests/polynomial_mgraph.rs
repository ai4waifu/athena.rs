//! M-Graph 多项式缓存与 witness。

use athena_engine::{
    CoefficientDomain, MonomialOrder, Number, PolynomialBuilder, PolynomialCacheOp, PolynomialRequest, PolynomialResult,
    Session, SymbolId,
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
    let k_add = athena_engine::cache_key_for_request(&PolynomialRequest::Add { lhs: p.clone(), rhs: q.clone() }, &session.rings).unwrap();
    let k_mul = athena_engine::cache_key_for_request(&PolynomialRequest::Mul { lhs: p, rhs: q }, &session.rings).unwrap();
    assert_ne!(k_add.operation, k_mul.operation);
    assert_eq!(k_add.operation, PolynomialCacheOp::Add);
}

#[test]
fn groebner_result_records_witness() {
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
        limits: athena_engine::GroebnerLimits::default(),
    };
    let r = session.execute_polynomial_mgraph(req.clone());
    assert!(matches!(r, PolynomialResult::Exact { .. }));
    assert_eq!(session.mgraph.polynomial.len(), 1);
    let entry = session.mgraph.polynomial.get(&athena_engine::cache_key_for_request(&req, &session.rings).unwrap()).unwrap();
    assert!(entry.witness.groebner_steps.is_some());
}

#[test]
fn jit_parity_eager_only_by_default() {
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
