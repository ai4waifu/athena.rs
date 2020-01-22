//! Queued DomainPlan execution (Living `29` · pending_plans bootstrap).
//!
//! Plans are queued without forging facts. Execution still goes through domain
//! providers and [`AdmissionGate`] admit helpers — never direct ExactUF writes.

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        dispatch::{DomainRequest, DomainResult, execute_domain},
        planner::{DomainPlan, PlanStep},
        polynomial::execute_polynomial_mgraph,
    },
    reasoning::mgraph::{
        obligation::ProofObligation,
        semantic_entry::try_admit_calculus_exact,
    },
    runtime::session::Session,
};

/// A Reflector-selected plan waiting for a bound [`DomainRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedPlan {
    /// PlanIR steps (must include `CallDomainProvider` to run).
    pub plan: DomainPlan,
    /// Obligation that produced `NeedComputation`.
    pub obligation: ProofObligation,
}

/// Execute one queued plan with a caller-bound request (AdmissionGate on exact results).
pub fn execute_queued_plan(
    session: &mut Session,
    queued: &QueuedPlan,
    request: DomainRequest,
) -> Result<DomainResult, Diagnostic> {
    if !queued
        .plan
        .steps
        .iter()
        .any(|s| matches!(s, PlanStep::CallDomainProvider))
    {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "pending_plans")
            .detail("reason", "plan missing CallDomainProvider"));
    }
    let result = match request {
        DomainRequest::Polynomial(req) => {
            let poly = execute_polynomial_mgraph(
                req,
                &session.rings,
                &session.polynomial_objects,
                &mut session.mgraph,
            );
            DomainResult::Polynomial(poly)
        }
        other => {
            let result = execute_domain(session, other)?;
            try_admit_calculus_exact(session, &queued.obligation, &result);
            result
        }
    };
    Ok(result)
}

/// Pop and execute the front queued plan with a bound request.
///
/// Returns `Ok(None)` when the queue is empty. On provider/admit errors the plan
/// stays at the front of the queue.
pub fn run_next_queued_plan(
    session: &mut Session,
    request: DomainRequest,
) -> Result<Option<DomainResult>, Diagnostic> {
    let Some(queued) = session.mgraph.operational.pending_plans.first().cloned() else {
        return Ok(None);
    };
    if !queued
        .plan
        .steps
        .iter()
        .any(|s| matches!(s, PlanStep::CallDomainProvider))
    {
        let _ = session.mgraph.operational.pending_plans.remove(0);
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "pending_plans")
            .detail("reason", "plan missing CallDomainProvider"));
    }
    let result = execute_queued_plan(session, &queued, request)?;
    let _ = session.mgraph.operational.pending_plans.remove(0);
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domains::planner::{DomainPlan, PlanStep},
        reasoning::mgraph::{ProofObligation, ScopeRef, predicates},
        runtime::session::Session,
    };

    #[test]
    fn run_next_queued_plan_executes_polynomial_and_admits() {
        use crate::domains::polynomial::{
            CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialRequest,
        };
        use athena_types::SymbolId;

        let mut session = Session::new();
        let ring = session
            .rings
            .intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex)
            .expect("ring");
        let polynomial = PolynomialBuilder::new(ring).build(&session.rings).expect("zero poly");
        let poly_ref = session.polynomial_objects.intern(polynomial, &session.rings);
        session.mgraph.operational.pending_plans.push(QueuedPlan {
            plan: DomainPlan {
                steps: vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult],
            },
            obligation: ProofObligation {
                predicate: predicates::POLYNOMIAL_RESULT,
                scope: ScopeRef::UNCONDITIONAL,
                known_objects: vec![],
            },
        });
        let result = run_next_queued_plan(
            &mut session,
            DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref }),
        )
        .expect("run")
        .expect("some");
        assert!(matches!(result, DomainResult::Polynomial(_)));
        assert!(session.mgraph.operational.pending_plans.is_empty());
        assert!(session.mgraph.semantic.relation_count() >= 1);
    }

    #[test]
    fn empty_queue_returns_none() {
        use crate::domains::polynomial::{
            CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialRequest,
        };
        use athena_types::SymbolId;

        let mut session = Session::new();
        let ring = session
            .rings
            .intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex)
            .expect("ring");
        let polynomial = PolynomialBuilder::new(ring).build(&session.rings).expect("zero poly");
        let poly_ref = session.polynomial_objects.intern(polynomial, &session.rings);
        let out = run_next_queued_plan(
            &mut session,
            DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref }),
        )
        .expect("ok");
        assert!(out.is_none());
    }
}
