//! `DomainPlan` step interpreter (Living `28` / `29`).
//!
//! Reflector / domain dispatch walk [`DomainPlan`] steps instead of treating
//! `CallDomainProvider` as the only real action with the rest as comments.

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        calculus::{CalculusResult, CalculusValue},
        dispatch::{DomainRequest, DomainResult},
        plan_normalize::normalize_domain_request,
        plan_select::select_domain_representation,
        planner::{DomainPlan, PlanStep},
        verify_replay::{VerifySnapshot, verify_recompute_domain_result},
        views::SeriesPolynomialView,
    },
    runtime::session::Session,
};

/// Audit trail from interpreting one [`DomainPlan`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanStepReport {
    /// Steps that successfully ran, in order.
    pub executed: Vec<PlanStep>,
    /// Whether [`PlanStep::Normalize`] ran (validation / coercion).
    pub normalized: bool,
    /// Whether Normalize rewrote at least one polynomial handle.
    pub normalize_coerced: bool,
    /// Whether [`PlanStep::SelectRepresentation`] ran.
    pub representation_selected: bool,
    /// Selected representation family label (when SelectRepresentation ran).
    pub selected_representation: Option<&'static str>,
    /// Whether [`PlanStep::CallDomainProvider`] ran.
    pub provider_invoked: bool,
    /// Whether [`PlanStep::Verify`] ran.
    pub verified: bool,
    /// Whether [`PlanStep::MaterializeResult`] ran.
    pub materialized: bool,
    /// Whether [`PlanStep::EmitResidual`] ran.
    pub residual_emitted: bool,
    /// Whether [`PlanStep::CrossDomainView`] opened a view.
    pub cross_domain_view: bool,
}

/// Run [`DomainPlan`] steps with a single provider callback (invoked at most once).
///
/// Bootstrap semantics:
/// - `Normalize` — validate DomainObject / term handles and coerce polynomial refs
///   onto canonical interned identities. Refreshes the Verify snapshot.
/// - `SelectRepresentation` — acknowledge the active representation family for the request.
/// - `CallDomainProvider` — invoke `provider` exactly once.
/// - `CrossDomainView` — open TypedView after provider output exists.
/// - `Verify` — independent domain recompute against claimed result.
/// - `MaterializeResult` — seal the `DomainResult` for the host.
/// - `EmitResidual` — allow completion alongside or instead of materialize.
pub fn interpret_domain_plan<F>(
    session: &mut Session,
    plan: &DomainPlan,
    request: DomainRequest,
    provider: F,
) -> Result<(DomainResult, PlanStepReport), Diagnostic>
where
    F: FnOnce(&mut Session, DomainRequest) -> Result<DomainResult, Diagnostic>,
{
    if !plan.steps.iter().any(|s| matches!(s, PlanStep::CallDomainProvider)) {
        return Err(plan_err("plan_missing_CallDomainProvider"));
    }
    if !plan.steps.iter().any(|s| matches!(s, PlanStep::MaterializeResult | PlanStep::EmitResidual)) {
        return Err(plan_err("plan_missing_MaterializeResult_or_EmitResidual"));
    }

    let mut report = PlanStepReport::default();
    let mut result: Option<DomainResult> = None;
    let mut pending_request = Some(request);
    let mut verify_snapshot = VerifySnapshot::from_request(pending_request.as_ref().expect("pending request"));
    let mut provider = Some(provider);

    for step in &plan.steps {
        match *step {
            PlanStep::Normalize => {
                let req = pending_request.take().ok_or_else(|| plan_err("request_already_consumed"))?;
                let outcome = normalize_domain_request(session, req)?;
                verify_snapshot = VerifySnapshot::from_request(&outcome.request);
                report.normalized = true;
                report.normalize_coerced = outcome.coerced;
                pending_request = Some(outcome.request);
                report.executed.push(*step);
            }
            PlanStep::SelectRepresentation => {
                let req = pending_request.as_ref().ok_or_else(|| plan_err("request_already_consumed"))?;
                let selected = select_domain_representation(session, req)?;
                report.representation_selected = true;
                report.selected_representation = Some(selected.family);
                report.executed.push(*step);
            }
            PlanStep::CallDomainProvider => {
                if report.provider_invoked {
                    return Err(plan_err("duplicate_CallDomainProvider"));
                }
                let req = pending_request.take().ok_or_else(|| plan_err("request_already_consumed"))?;
                let call = provider.take().ok_or_else(|| plan_err("provider_callback_missing"))?;
                result = Some(call(session, req)?);
                report.provider_invoked = true;
                report.executed.push(*step);
            }
            PlanStep::CrossDomainView => {
                let current = result.as_ref().ok_or_else(|| plan_err("CrossDomainView_before_provider"))?;
                open_cross_domain_view(session, current)?;
                report.cross_domain_view = true;
                report.executed.push(*step);
            }
            PlanStep::Verify => {
                let current = result.as_ref().ok_or_else(|| plan_err("Verify_before_provider"))?;
                verify_recompute_domain_result(session, &verify_snapshot, current)?;
                report.verified = true;
                report.executed.push(*step);
            }
            PlanStep::MaterializeResult => {
                if result.is_none() {
                    return Err(plan_err("MaterializeResult_without_provider_result"));
                }
                report.materialized = true;
                report.executed.push(*step);
            }
            PlanStep::EmitResidual => {
                report.residual_emitted = true;
                report.executed.push(*step);
            }
        }
    }

    if !report.materialized && !report.residual_emitted {
        return Err(plan_err("plan_did_not_complete_Materialize_or_EmitResidual"));
    }
    let out = result.ok_or_else(|| plan_err("plan_completed_without_DomainResult"))?;
    Ok((out, report))
}

fn plan_err(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "plan_exec").detail("reason", reason)
}

pub(crate) fn open_cross_domain_view(session: &Session, result: &DomainResult) -> Result<(), Diagnostic> {
    match result {
        DomainResult::Calculus(
            CalculusResult::Exact { value: CalculusValue::Series(series_ref), .. }
            | CalculusResult::Conditional { value: CalculusValue::Series(series_ref), .. }
            | CalculusResult::Unevaluated { expression: CalculusValue::Series(series_ref), .. },
        ) => {
            SeriesPolynomialView::open(&session.series_objects, *series_ref).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "views")
                    .detail("reason", "missing_series_ref_for_cross_domain_view")
                    .arg("ref", series_ref.0)
            })?;
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::{
        calculus::{CalculusRequest, DerivativeOrder},
        planner::plan_domain,
    };
    use athena_ir::{Atom, TermNode};
    use athena_types::{AssumptionSet, SourceSpan, SymbolId, TermId};

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
        let (result, report) = interpret_domain_plan(&mut session, &plan, request, |s, r| crate::domains::dispatch::call_domain_provider(s, r))
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
        let err = interpret_domain_plan(&mut session, &plan, request, |s, r| crate::domains::dispatch::call_domain_provider(s, r))
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
            Ok(DomainResult::Calculus(crate::domains::calculus::CalculusResult::Exact {
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
            interpret_domain_plan(&mut session, &plan, request, |s, r| crate::domains::dispatch::call_domain_provider(s, r))
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
        let err = interpret_domain_plan(&mut session, &plan, request, |s, r| crate::domains::dispatch::call_domain_provider(s, r))
            .expect_err("missing");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("missing_term_id"));
    }

    #[test]
    fn normalize_rejects_missing_polynomial_ref() {
        use crate::domains::polynomial::{PolynomialRef, PolynomialRequest};

        let mut session = Session::new();
        let request = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: PolynomialRef(99) });
        let plan = DomainPlan { steps: vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::MaterializeResult] };
        let err = interpret_domain_plan(&mut session, &plan, request, |s, r| crate::domains::dispatch::call_domain_provider(s, r))
            .expect_err("missing");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("missing_polynomial_ref"));
    }

    #[test]
    fn normalize_rejects_missing_matrix_ref() {
        use crate::domains::linear_algebra::{LinearAlgebraRequest, MatrixRef};

        let mut session = Session::new();
        let request = DomainRequest::LinearAlgebra(LinearAlgebraRequest::Det { matrix: MatrixRef(7) });
        let plan = DomainPlan { steps: vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::MaterializeResult] };
        let err = interpret_domain_plan(&mut session, &plan, request, |s, r| crate::domains::dispatch::call_domain_provider(s, r))
            .expect_err("missing");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("missing_matrix_ref"));
    }

    #[test]
    fn normalize_accepts_existing_polynomial_ref() {
        use crate::domains::polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialRequest};
        use athena_types::SymbolId;

        let mut session = Session::new();
        let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).expect("ring");
        let poly = PolynomialBuilder::new(ring).build(&session.rings).expect("zero");
        let poly_ref = session.polynomial_objects.intern(poly, &session.rings);
        let request = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref });
        let plan = DomainPlan { steps: vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::Verify, PlanStep::MaterializeResult] };
        let (_result, report) =
            interpret_domain_plan(&mut session, &plan, request, |s, r| crate::domains::dispatch::call_domain_provider(s, r)).expect("ok");
        assert!(report.normalized && report.provider_invoked && report.verified);
    }
}
