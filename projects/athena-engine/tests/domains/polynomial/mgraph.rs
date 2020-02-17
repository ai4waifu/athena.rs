//! M-Graph 多项式缓存、admission gate 与 JIT parity 矩阵。

use athena_engine::{
    domains::polynomial::{
        CoefficientDomain, GroebnerLimits, MonomialOrder, PolynomialBuilder, PolynomialDomainValue, PolynomialRequest, PolynomialResult,
        cache_key_for_request, record_polynomial_result,
    },
    reasoning::mgraph::{
        AdmissionOutcome, AdmissionRejectReason, Claim, Evidence, EvidenceVerifier, Guarantee, Scope, VerificationPolicy,
        admit_polynomial_result, proposition_from_cache_key,
    },
    runtime::Session,
};
use athena_numeric::Number;
use athena_types::SymbolId;

fn z_x_ring(session: &mut Session) -> athena_engine::domains::polynomial::RingId {
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
    let lhs = session.polynomial_objects.intern(a, &session.rings);
    let rhs = session.polynomial_objects.intern(c, &session.rings);
    let req = PolynomialRequest::Add { lhs, rhs };
    let r1 = session.execute_polynomial_mgraph(req.clone());
    assert!(matches!(r1, PolynomialResult::Exact { .. }));
    assert_eq!(session.mgraph.operational.result_cache.polynomial.len(), 1);
    assert_eq!(session.mgraph.semantic.derived.rewrite_witnesses.len(), 1);
    assert_eq!(session.mgraph.semantic.admission_journal.count(), 1);
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
    let generator = session.polynomial_objects.intern(g, &session.rings);
    let req = PolynomialRequest::Groebner { generators: vec![generator], limits: GroebnerLimits::default() };
    session.execute_polynomial_mgraph(req.clone());
    assert_eq!(session.mgraph.semantic.admission_journal.count(), 1);
    let key = cache_key_for_request(&req, &session.rings, &session.polynomial_objects).unwrap();
    let vc = session.mgraph.semantic.admission_journal.get(athena_engine::reasoning::mgraph::FactId(0)).unwrap();
    assert_eq!(vc.claim.guarantee, Guarantee::ProvenExact);
    assert!(session.mgraph.operational.result_cache.polynomial.get(&key).unwrap().witness.is_some());
}

#[test]
fn groebner_partial_cached_but_not_admitted() {
    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    // Non-coprime leading monomials so max_s_pairs=0 still yields Partial.
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(1), vec![2, 0]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0, 1]).unwrap();
    let g1 = b1.build(&session.rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(1), vec![1, 1]).unwrap();
    b2.push_term(Number::small_int(-1), vec![0, 0]).unwrap();
    let g2 = b2.build(&session.rings).unwrap();
    let r1 = session.polynomial_objects.intern(g1, &session.rings);
    let r2 = session.polynomial_objects.intern(g2, &session.rings);
    let req = PolynomialRequest::Groebner { generators: vec![r1, r2], limits: GroebnerLimits { max_s_pairs: 0, max_basis_size: 128 } };
    session.execute_polynomial_mgraph(req.clone());
    assert_eq!(session.mgraph.semantic.admission_journal.count(), 0);
    assert_eq!(session.mgraph.operational.result_cache.polynomial.partial_len(), 1);
    let key = cache_key_for_request(&req, &session.rings, &session.polynomial_objects).unwrap();
    match admit_polynomial_result(&key, &session.mgraph.operational.result_cache.polynomial.get_partial(&key).unwrap().result) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::GroebnerIncomplete, guarantee: Guarantee::Partial } => {}
        other => panic!("expected GroebnerIncomplete, got {other:?}"),
    }
}

#[test]
fn placeholder_exact_result_not_admitted() {
    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let poly = session.polynomial_objects.intern(PolynomialBuilder::new(ring).build(&session.rings).unwrap(), &session.rings);
    let key = cache_key_for_request(&PolynomialRequest::Normalize { polynomial: poly }, &session.rings, &session.polynomial_objects).unwrap();
    record_polynomial_result(
        key.clone(),
        PolynomialResult::Exact { value: PolynomialDomainValue::Placeholder },
        &mut session.mgraph,
        Some(&session.rings),
    )
    .unwrap();
    assert_eq!(session.mgraph.semantic.admission_journal.count(), 0);
    match admit_polynomial_result(&key, &session.mgraph.operational.result_cache.polynomial.get_partial(&key).unwrap().result) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::Placeholder, .. } => {}
        other => panic!("expected Placeholder, got {other:?}"),
    }
}

#[test]
fn probable_claim_blocked_by_verifier() {
    let mut session = Session::default();
    let ring = z_x_ring(&mut session);
    let lhs = session.polynomial_objects.intern(PolynomialBuilder::new(ring).build(&session.rings).unwrap(), &session.rings);
    let rhs = session.polynomial_objects.intern(PolynomialBuilder::new(ring).build(&session.rings).unwrap(), &session.rings);
    let key = cache_key_for_request(&PolynomialRequest::Add { lhs, rhs }, &session.rings, &session.polynomial_objects).unwrap();
    let claim = Claim {
        proposition: proposition_from_cache_key(&key),
        scope: Scope::Unconditional,
        guarantee: Guarantee::Probable,
        evidence: Evidence::TrustedKernel {
            provider: athena_engine::reasoning::mgraph::POLYNOMIAL_PROVIDER_ID,
            certificate: athena_engine::reasoning::mgraph::EvidenceCertificate::TestHarness,
            summary: "probable".into(),
        },
    };
    match EvidenceVerifier::verify(&claim, &VerificationPolicy::default()) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::ProbableResult, guarantee: Guarantee::Probable } => {}
        other => panic!("expected ProbableResult, got {other:?}"),
    }
}

#[test]
fn forged_verified_groebner_rejected_by_independent_replay() {
    use athena_engine::{
        domains::{
            algebra::{PropertyState, PropertyWitness},
            polynomial::{
                GroebnerAlgorithm, GroebnerBasisValue, GroebnerCertificate, GroebnerStatus, PolynomialDomainValue, PolynomialRequest,
                PolynomialResult, cache_key_for_request,
            },
        },
        reasoning::mgraph::{AdmissionOutcome, AdmissionRejectReason, admit_polynomial_result_with_rings},
    };

    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    // 经典非 Gröbner 基，却伪造 Verified 证书。
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(1), vec![2, 0]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0, 1]).unwrap();
    let g1 = b1.build(&session.rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(1), vec![1, 1]).unwrap();
    b2.push_term(Number::small_int(-1), vec![0, 0]).unwrap();
    let g2 = b2.build(&session.rings).unwrap();
    let r1 = session.polynomial_objects.intern(g1.clone(), &session.rings);
    let r2 = session.polynomial_objects.intern(g2.clone(), &session.rings);
    let req = PolynomialRequest::Groebner { generators: vec![r1, r2], limits: GroebnerLimits::default() };
    let key = cache_key_for_request(&req, &session.rings, &session.polynomial_objects).unwrap();
    let forged = PolynomialResult::Exact {
        value: PolynomialDomainValue::GroebnerBasis(GroebnerBasisValue {
            ring,
            basis: vec![g1, g2],
            certificate: GroebnerCertificate {
                algorithm: GroebnerAlgorithm::Buchberger,
                ring,
                input_generators: 2,
                basis_elements: 2,
                s_pair_steps: 1,
                complete: true,
                verification: PropertyState::Proven { value: (), witness: PropertyWitness::placeholder("forged") },
                elimination_elements: None,
            },
            status: GroebnerStatus::Verified,
            pending_pairs: Vec::new(),
            pending_insertion: None,
            candidate_sugars: None,
            pending_insertion_sugar: None,
        }),
    };
    match admit_polynomial_result_with_rings(&key, &forged, &session.rings) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, .. } => {}
        other => panic!("expected EvidenceMismatch, got {other:?}"),
    }
}

#[test]
fn groebner_second_goal_is_already_known_after_admit() {
    use athena_engine::{
        api::DomainGoal,
        domains::{
            dispatch::{DomainRequest, DomainResult},
            polynomial::{GroebnerLimits, PolynomialDomainValue, PolynomialRequest, PolynomialResult},
        },
        reasoning::mgraph::{DomainSemanticOutcome, domain_result_from_semantic_outcome, execute_domain_goal},
    };

    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![1]).unwrap();
    b.push_term(Number::small_int(-1), vec![0]).unwrap();
    let g = b.build(&session.rings).unwrap();
    let generator = session.polynomial_objects.intern(g, &session.rings);
    let make_goal = || {
        DomainGoal::Dispatch(DomainRequest::Polynomial(PolynomialRequest::Groebner {
            generators: vec![generator],
            limits: GroebnerLimits::default(),
        }))
    };
    let first = execute_domain_goal(&mut session, make_goal()).expect("first");
    let DomainSemanticOutcome::Computed(DomainResult::Polynomial(PolynomialResult::Exact {
        value: PolynomialDomainValue::GroebnerBasis(first_gb),
    })) = &first
    else {
        panic!("expected Exact GroebnerBasis first, got {first:?}");
    };
    assert!(first_gb.is_exact_witness());
    assert!(session.mgraph.semantic.relation_count() >= 1);
    let second = execute_domain_goal(&mut session, make_goal()).expect("second");
    match second {
        DomainSemanticOutcome::AlreadyKnown { relation } => {
            let replayed =
                domain_result_from_semantic_outcome(&session, DomainSemanticOutcome::AlreadyKnown { relation }).expect("materialize");
            match replayed {
                DomainResult::Polynomial(PolynomialResult::Exact { value: PolynomialDomainValue::GroebnerBasis(gb) }) => {
                    assert!(gb.is_exact_witness());
                    assert_eq!(gb.basis.len(), first_gb.basis.len());
                }
                other => panic!("expected materialize GroebnerBasis, got {other:?}"),
            }
        }
        other => panic!("expected AlreadyKnown, got {other:?}"),
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
    let (prod, outcome) = athena_engine::domains::polynomial::mul_with_jit_parity(lhs, rhs, &session.rings).unwrap();
    assert_eq!(prod.terms().len(), 1);
    assert_eq!(outcome, athena_engine::domains::polynomial::JitParityOutcome::EagerOnly);
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
    let (_, outcome) = athena_engine::domains::polynomial::mul_with_jit_parity(lhs, rhs, &session.rings).unwrap();
    assert_eq!(outcome, athena_engine::domains::polynomial::JitParityOutcome::JitUnavailable);
}
