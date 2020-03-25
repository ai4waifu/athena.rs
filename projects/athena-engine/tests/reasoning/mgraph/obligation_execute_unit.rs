//! 自 `src/reasoning/mgraph/obligation/execute.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::{
        DomainRequest, DomainResult,
        planner::{DomainPlan, PlanStep},
        polynomial::PolynomialRequest,
    },
    reasoning::mgraph::{ProofObligation, ScopeRef, obligation::*, predicates},
};
use athena_ir::fnv1a64;
use athena_types::{Diagnostic, DiagnosticCode};

fn poly_session() -> (Session, athena_engine::domains::polynomial::PolynomialRef) {
    use athena_engine::domains::polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder};
    use athena_types::SymbolId;

    let mut session = Session::new();
    let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).expect("ring");
    let polynomial = PolynomialBuilder::new(ring).build(&session.rings).expect("zero poly");
    let poly_ref = session.polynomial_objects.intern(polynomial, &session.rings);
    (session, poly_ref)
}

#[test]
fn run_next_queued_plan_executes_polynomial_and_admits() {
    use athena_engine::domains::polynomial::PolynomialRequest;

    let (mut session, poly_ref) = poly_session();
    let request = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref });
    let obligation = ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: ScopeRef::UNCONDITIONAL, known_objects: vec![] };
    session.mgraph.operational.pending_plans.push(QueuedPlan::bound(
        &session,
        DomainPlan { steps: vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult] },
        obligation,
        &request,
    ));
    let result = run_next_queued_plan(&mut session, request).expect("run").expect("some");
    assert!(matches!(result, DomainResult::Polynomial(_)));
    assert!(session.mgraph.operational.pending_plans.is_empty());
    assert!(session.mgraph.semantic.relation_count() >= 1);
}

#[test]
fn fingerprint_binding_rejects_mismatched_request() {
    let (mut session, poly_ref) = poly_session();
    let bound_req = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref });
    let obligation = ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: ScopeRef::UNCONDITIONAL, known_objects: vec![] };
    session.mgraph.operational.pending_plans.push(QueuedPlan::bound(
        &session,
        DomainPlan { steps: vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult] },
        obligation,
        &bound_req,
    ));
    // 不同环/多项式身份 → 尽量不同指纹；用第二条多项式构造 Add 形态错配。
    let ring2 = session
        .rings
        .intern(
            athena_engine::domains::polynomial::CoefficientDomain::Integer,
            vec![athena_types::SymbolId(1)],
            athena_engine::domains::polynomial::MonomialOrder::Lex,
        )
        .expect("ring2");
    let poly2 = athena_engine::domains::polynomial::PolynomialBuilder::new(ring2).build(&session.rings).expect("poly2");
    let poly_ref2 = session.polynomial_objects.intern(poly2, &session.rings);
    let mismatch = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref2 });
    let err = run_next_queued_plan(&mut session, mismatch).expect_err("mismatch");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("plan_binding_mismatch"));
    assert_eq!(session.mgraph.operational.pending_plans.len(), 1);
}

#[test]
fn empty_queue_returns_none() {
    let (mut session, poly_ref) = poly_session();
    let out = run_next_queued_plan(&mut session, DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref })).expect("ok");
    assert!(out.is_none());
}

#[test]
fn run_queued_plans_drains_matching_requests() {
    let (mut session, poly_ref) = poly_session();
    let request = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref });
    let obligation = ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: ScopeRef::UNCONDITIONAL, known_objects: vec![] };
    let plan = QueuedPlan::bound(
        &session,
        DomainPlan { steps: vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult] },
        obligation,
        &request,
    );
    session.mgraph.operational.pending_plans.push(plan.owning_copy());
    session.mgraph.operational.pending_plans.push(plan);
    let report = run_queued_plans(
        &mut session,
        [
            DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref }),
            DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref }),
        ],
    )
    .expect("batch");
    assert_eq!(report.executed, 2);
    assert_eq!(report.remaining, 0);
    assert_eq!(report.results.len(), 2);
    assert!(session.mgraph.operational.pending_plans.is_empty());
}
