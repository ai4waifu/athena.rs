//! Queued DomainPlan execution (Living `29` · pending_plans bootstrap).
//!
//! Plans are queued without forging facts. Execution still goes through domain
//! providers and [`AdmissionGate`] admit helpers — never direct ExactUF writes.
//!
//! [`PlanBinding`] ties a queued plan to a verifiable request fingerprint so a
//! caller-supplied [`DomainRequest`] cannot silently mismatch the obligation.

use athena_ir::fnv1a64;
use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        calculus::CalculusRequest,
        dispatch::{DomainRequest, DomainResult, call_domain_provider},
        plan_exec::interpret_domain_plan,
        planner::DomainPlan,
        polynomial::{cache_key_for_request, execute_polynomial_mgraph},
    },
    reasoning::mgraph::{
        core::refs::{PredicateId, TheoryContextId, predicates},
        obligation::ProofObligation,
        semantic_entry::try_admit_calculus_exact,
    },
    runtime::session::Session,
};

/// Verifiable link from a queued plan to the DomainRequest that may execute it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PlanBinding {
    /// Wake-scheduled plan without a request yet (caller must bind carefully).
    #[default]
    Unbound,
    /// Stable fingerprint gate: execute only if the supplied request matches.
    Fingerprint {
        /// Theory context expected for the request.
        theory: TheoryContextId,
        /// Predicate expected on the obligation / relation family.
        predicate: PredicateId,
        /// Request identity fingerprint (domain-defined).
        request_fingerprint: u64,
    },
}

/// A Reflector-selected plan waiting for a bound [`DomainRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedPlan {
    /// `DomainPlan` steps (must include `CallDomainProvider` to run).
    pub plan: DomainPlan,
    /// Obligation that produced `NeedComputation`.
    pub obligation: ProofObligation,
    /// Request/goal binding (fingerprint gate or unbound).
    pub binding: PlanBinding,
}

impl QueuedPlan {
    /// Queue a plan without a request fingerprint (wake path).
    pub fn unbound(plan: DomainPlan, obligation: ProofObligation) -> Self {
        Self { plan, obligation, binding: PlanBinding::Unbound }
    }

    /// Queue a plan with a fingerprint derived from `request`.
    pub fn bound(session: &Session, plan: DomainPlan, obligation: ProofObligation, request: &DomainRequest) -> Self {
        let binding = plan_binding_for_request(session, request, &obligation).unwrap_or(PlanBinding::Unbound);
        Self { plan, obligation, binding }
    }
}

/// Derive a [`PlanBinding`] fingerprint for calculus / polynomial requests.
pub fn plan_binding_for_request(session: &Session, request: &DomainRequest, obligation: &ProofObligation) -> Option<PlanBinding> {
    match request {
        DomainRequest::Polynomial(req) => {
            let key = cache_key_for_request(req, &session.rings, &session.polynomial_objects).ok()?;
            Some(PlanBinding::Fingerprint {
                theory: TheoryContextId::POLYNOMIAL,
                predicate: predicates::POLYNOMIAL_RESULT,
                request_fingerprint: key.fingerprint(),
            })
        }
        DomainRequest::Calculus(calc) => {
            let (predicate, fingerprint) = calculus_binding(calc)?;
            Some(PlanBinding::Fingerprint { theory: TheoryContextId::CALCULUS, predicate, request_fingerprint: fingerprint })
        }
        _ => {
            // Fallback: mix obligation identity so unbound domains still get a gate
            // when an obligation exists with known objects.
            if obligation.known_objects.is_empty() {
                return None;
            }
            let mut state = fnv1a64(b"athena.plan-binding.obligation");
            mix_u64(&mut state, u64::from(obligation.predicate.0));
            mix_u64(&mut state, u64::from(obligation.scope.0));
            for obj in &obligation.known_objects {
                mix_u64(&mut state, u64::from(obj.theory.0));
                mix_u64(&mut state, obj.fingerprint);
            }
            Some(PlanBinding::Fingerprint {
                theory: obligation.known_objects.first().map(|o| o.theory).unwrap_or(TheoryContextId::DEFAULT),
                predicate: obligation.predicate,
                request_fingerprint: state,
            })
        }
    }
}

fn calculus_binding(request: &CalculusRequest) -> Option<(PredicateId, u64)> {
    let mut state = fnv1a64(b"athena.plan-binding.calculus");
    match request {
        CalculusRequest::Derivative { expression, variable, order, .. } => {
            mix_u64(&mut state, u64::from(expression.0));
            mix_u64(&mut state, u64::from(variable.0));
            mix_u64(&mut state, derivative_order_tag(*order));
            Some((predicates::DERIVATIVE_OF, state))
        }
        CalculusRequest::Integral { expression, variable, .. } | CalculusRequest::DefiniteIntegral { expression, variable, .. } => {
            mix_u64(&mut state, u64::from(expression.0));
            mix_u64(&mut state, u64::from(variable.0));
            Some((predicates::INTEGRAL_OF, state))
        }
        CalculusRequest::Series { expression, variable, .. }
        | CalculusRequest::Laurent { expression, variable, .. }
        | CalculusRequest::Asymptotic { expression, variable, .. } => {
            mix_u64(&mut state, u64::from(expression.0));
            mix_u64(&mut state, u64::from(variable.0));
            Some((predicates::SERIES_EXPANSION, state))
        }
        _ => None,
    }
}

fn derivative_order_tag(order: crate::domains::calculus::DerivativeOrder) -> u64 {
    match order {
        crate::domains::calculus::DerivativeOrder::First => 1,
        crate::domains::calculus::DerivativeOrder::Repeated(n) => 2 + u64::from(n),
    }
}

fn mix_u64(state: &mut u64, v: u64) {
    *state ^= v;
    *state = state.wrapping_mul(0x0000_0100_0000_01b3);
}

/// Reject requests that do not match a fingerprint-gated queued plan.
pub fn verify_plan_binding(
    session: &Session,
    binding: &PlanBinding,
    obligation: &ProofObligation,
    request: &DomainRequest,
) -> Result<(), Diagnostic> {
    let PlanBinding::Fingerprint { theory, predicate, request_fingerprint } = binding
    else {
        return Ok(());
    };
    if obligation.predicate != *predicate {
        return Err(binding_mismatch("obligation_predicate"));
    }
    let Some(PlanBinding::Fingerprint { theory: got_theory, predicate: got_predicate, request_fingerprint: got_fp }) =
        plan_binding_for_request(session, request, obligation)
    else {
        return Err(binding_mismatch("request_unfingerprintable"));
    };
    if got_theory != *theory || got_predicate != *predicate || got_fp != *request_fingerprint {
        return Err(binding_mismatch("request_fingerprint"));
    }
    Ok(())
}

fn binding_mismatch(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "pending_plans")
        .detail("reason", "plan_binding_mismatch")
        .detail("detail", reason)
}

/// Execute one queued plan with a caller-bound request (AdmissionGate on exact results).
///
/// Walks [`DomainPlan`] via [`interpret_domain_plan`]. Polynomial provider uses
/// `execute_polynomial_mgraph`; calculus exact results admit after materialize.
pub fn execute_queued_plan(session: &mut Session, queued: &QueuedPlan, request: DomainRequest) -> Result<DomainResult, Diagnostic> {
    verify_plan_binding(session, &queued.binding, &queued.obligation, &request)?;
    let obligation = queued.obligation.clone();
    let (result, _report) = interpret_domain_plan(session, &queued.plan, request, |session, req| match req {
        DomainRequest::Polynomial(poly_req) => {
            let poly = execute_polynomial_mgraph(poly_req, &session.rings, &session.polynomial_objects, &mut session.mgraph);
            Ok(DomainResult::Polynomial(poly))
        }
        other => call_domain_provider(session, other),
    })?;
    try_admit_calculus_exact(session, &obligation, &result);
    Ok(result)
}

/// Pop and execute the front queued plan with a bound request.
///
/// Returns `Ok(None)` when the queue is empty. On provider/admit/binding errors the
/// plan stays at the front of the queue (except malformed plans missing CallDomainProvider).
pub fn run_next_queued_plan(session: &mut Session, request: DomainRequest) -> Result<Option<DomainResult>, Diagnostic> {
    let Some(queued) = session.mgraph.operational.pending_plans.first().cloned()
    else {
        return Ok(None);
    };
    match execute_queued_plan(session, &queued, request) {
        Ok(result) => {
            let _ = session.mgraph.operational.pending_plans.remove(0);
            Ok(Some(result))
        }
        Err(err) => {
            // Drop malformed plans that the interpreter rejects for structure.
            let reason = err.details.get("reason").map(|v| v.to_string());
            if matches!(reason.as_deref(), Some("plan_missing_CallDomainProvider") | Some("plan_missing_MaterializeResult_or_EmitResidual")) {
                let _ = session.mgraph.operational.pending_plans.remove(0);
            }
            Err(err)
        }
    }
}

/// Batch-execute queued plans, pairing each with the next bound request.
///
/// Stops on the first provider/binding error (that plan remains at the front). Extra
/// requests beyond the queue length are ignored. When requests run out, remaining
/// plans stay queued.
pub fn run_queued_plans(session: &mut Session, requests: impl IntoIterator<Item = DomainRequest>) -> Result<QueuedPlanBatchReport, Diagnostic> {
    let mut report = QueuedPlanBatchReport::default();
    for request in requests {
        if session.mgraph.operational.pending_plans.is_empty() {
            break;
        }
        match run_next_queued_plan(session, request)? {
            Some(result) => {
                report.executed = report.executed.saturating_add(1);
                report.results.push(result);
            }
            None => break,
        }
    }
    report.remaining = session.mgraph.operational.pending_plans.len() as u32;
    Ok(report)
}

/// Report from batch-executing queued plans.
#[derive(Debug, PartialEq, Default)]
pub struct QueuedPlanBatchReport {
    /// Plans executed successfully in this call.
    pub executed: u32,
    /// Plans still waiting in the queue.
    pub remaining: u32,
    /// Domain results in execution order.
    pub results: Vec<DomainResult>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domains::planner::{DomainPlan, PlanStep},
        reasoning::mgraph::{ProofObligation, ScopeRef, predicates},
        runtime::session::Session,
    };

    fn poly_session() -> (Session, crate::domains::polynomial::PolynomialRef) {
        use crate::domains::polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder};
        use athena_types::SymbolId;

        let mut session = Session::new();
        let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).expect("ring");
        let polynomial = PolynomialBuilder::new(ring).build(&session.rings).expect("zero poly");
        let poly_ref = session.polynomial_objects.intern(polynomial, &session.rings);
        (session, poly_ref)
    }

    #[test]
    fn run_next_queued_plan_executes_polynomial_and_admits() {
        use crate::domains::polynomial::PolynomialRequest;

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
        use crate::domains::polynomial::PolynomialRequest;

        let (mut session, poly_ref) = poly_session();
        let bound_req = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref });
        let obligation = ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: ScopeRef::UNCONDITIONAL, known_objects: vec![] };
        session.mgraph.operational.pending_plans.push(QueuedPlan::bound(
            &session,
            DomainPlan { steps: vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult] },
            obligation,
            &bound_req,
        ));
        // Different ring/poly identity → different fingerprint when possible; use Add-shaped mismatch via second poly.
        let ring2 = session
            .rings
            .intern(
                crate::domains::polynomial::CoefficientDomain::Integer,
                vec![athena_types::SymbolId(1)],
                crate::domains::polynomial::MonomialOrder::Lex,
            )
            .expect("ring2");
        let poly2 = crate::domains::polynomial::PolynomialBuilder::new(ring2).build(&session.rings).expect("poly2");
        let poly_ref2 = session.polynomial_objects.intern(poly2, &session.rings);
        let mismatch = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref2 });
        let err = run_next_queued_plan(&mut session, mismatch).expect_err("mismatch");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("plan_binding_mismatch"));
        assert_eq!(session.mgraph.operational.pending_plans.len(), 1);
    }

    #[test]
    fn empty_queue_returns_none() {
        use crate::domains::polynomial::PolynomialRequest;

        let (mut session, poly_ref) = poly_session();
        let out =
            run_next_queued_plan(&mut session, DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref })).expect("ok");
        assert!(out.is_none());
    }

    #[test]
    fn run_queued_plans_drains_matching_requests() {
        use crate::domains::polynomial::PolynomialRequest;

        let (mut session, poly_ref) = poly_session();
        let request = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref });
        let obligation = ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: ScopeRef::UNCONDITIONAL, known_objects: vec![] };
        let plan = QueuedPlan::bound(
            &session,
            DomainPlan { steps: vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult] },
            obligation,
            &request,
        );
        session.mgraph.operational.pending_plans.push(plan.clone());
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
}
