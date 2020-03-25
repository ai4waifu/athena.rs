//! 自 `src/domains/plan_exec.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::{
        calculus::{CalculusRequest, CalculusValue, DerivativeOrder},
        planner::plan_domain,
        *,
    },
};
use athena_ir::{Atom, TermNode};
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, SourceSpan, SymbolId, TermId};

fn push_number_term(session: &mut Session, n: i64) -> TermId {
    session.arena.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(n))), SourceSpan::default())
}

#[test]
fn interpret_runs_normalize_verify_materialize() {
    let mut session = Session::new();
    let expression = push_number_term(&mut session, 1);
    let request = DomainRequest::Calculus(CalculusRequest::Derivative {
        expression,
        variable: SymbolId(0),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    });
    let plan = DomainPlan { steps: vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::Verify, PlanStep::MaterializeResult] };
    let (result, report) =
        interpret_domain_plan(&mut session, &plan, request, |s, r| athena_engine::domains::dispatch::call_domain_provider(s, r))
            .expect("interpret");
    assert!(matches!(result, DomainResult::Calculus(_)));
    assert_eq!(report.executed, vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::Verify, PlanStep::MaterializeResult,]);
    assert!(report.provider_invoked && report.verified && report.materialized && report.normalized);
}

#[test]
fn interpret_rejects_verify_before_provider() {
    let mut session = Session::new();
    let expression = push_number_term(&mut session, 1);
    let request = DomainRequest::Calculus(CalculusRequest::Derivative {
        expression,
        variable: SymbolId(0),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    });
    let plan = DomainPlan { steps: vec![PlanStep::Verify, PlanStep::CallDomainProvider, PlanStep::MaterializeResult] };
    let err = interpret_domain_plan(&mut session, &plan, request, |s, r| athena_engine::domains::dispatch::call_domain_provider(s, r))
        .expect_err("order");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("Verify_before_provider"));
}

#[test]
fn interpret_rejects_forged_calculus_on_verify() {
    let mut session = Session::new();
    let expression = push_number_term(&mut session, 1);
    let request = DomainRequest::Calculus(CalculusRequest::Derivative {
        expression,
        variable: SymbolId(0),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    });
    let plan = DomainPlan { steps: vec![PlanStep::CallDomainProvider, PlanStep::Verify, PlanStep::MaterializeResult] };
    let err = interpret_domain_plan(&mut session, &plan, request, |_s, _r| {
        Ok(DomainResult::Calculus(athena_engine::domains::calculus::CalculusResult::Exact {
            value: CalculusValue::Expression(TermId(999_999)),
            conditions: Vec::new(),
        }))
    })
    .expect_err("forge");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("calculus_recompute_mismatch"));
}

#[test]
fn plan_domain_default_is_interpretable() {
    let mut session = Session::new();
    let expression = push_number_term(&mut session, 2);
    let request = DomainRequest::Calculus(CalculusRequest::Derivative {
        expression,
        variable: SymbolId(0),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    });
    let plan = plan_domain(&request);
    let (_result, report) =
        interpret_domain_plan(&mut session, &plan, request, |s, r| athena_engine::domains::dispatch::call_domain_provider(s, r))
            .expect("default plan");
    assert!(report.provider_invoked && report.verified && report.materialized);
    assert!(report.normalized);
    assert!(report.representation_selected);
    assert_eq!(report.selected_representation, Some("term_store"));
}

#[test]
fn normalize_rejects_missing_calculus_term() {
    let mut session = Session::new();
    let request = DomainRequest::Calculus(CalculusRequest::Derivative {
        expression: TermId(0),
        variable: SymbolId(0),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    });
    let plan = DomainPlan { steps: vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::MaterializeResult] };
    let err = interpret_domain_plan(&mut session, &plan, request, |s, r| athena_engine::domains::dispatch::call_domain_provider(s, r))
        .expect_err("missing");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("missing_term_id"));
}

#[test]
fn normalize_rejects_missing_polynomial_ref() {
    use athena_engine::domains::polynomial::{PolynomialRef, PolynomialRequest};

    let mut session = Session::new();
    let request = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: PolynomialRef(99) });
    let plan = DomainPlan { steps: vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::MaterializeResult] };
    let err = interpret_domain_plan(&mut session, &plan, request, |s, r| athena_engine::domains::dispatch::call_domain_provider(s, r))
        .expect_err("missing");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("missing_polynomial_ref"));
}

#[test]
fn normalize_rejects_missing_matrix_ref() {
    use athena_engine::domains::linear_algebra::{LinearAlgebraRequest, MatrixRef};

    let mut session = Session::new();
    let request = DomainRequest::LinearAlgebra(LinearAlgebraRequest::Det { matrix: MatrixRef(7) });
    let plan = DomainPlan { steps: vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::MaterializeResult] };
    let err = interpret_domain_plan(&mut session, &plan, request, |s, r| athena_engine::domains::dispatch::call_domain_provider(s, r))
        .expect_err("missing");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("missing_matrix_ref"));
}

#[test]
fn normalize_accepts_existing_polynomial_ref() {
    use athena_engine::domains::polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialRequest};
    use athena_types::SymbolId;

    let mut session = Session::new();
    let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).expect("ring");
    let poly = PolynomialBuilder::new(ring).build(&session.rings).expect("zero");
    let poly_ref = session.polynomial_objects.intern(poly, &session.rings);
    let request = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref });
    let plan = DomainPlan { steps: vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::Verify, PlanStep::MaterializeResult] };
    let (_result, report) =
        interpret_domain_plan(&mut session, &plan, request, |s, r| athena_engine::domains::dispatch::call_domain_provider(s, r)).expect("ok");
    assert!(report.normalized && report.provider_invoked && report.verified);
}
