//! PlanIR step interpreter (Living `28` / `29`).
//!
//! Reflector / domain dispatch walk [`DomainPlan`] steps instead of treating
//! `CallDomainProvider` as the only real action with the rest as comments.

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        calculus::{CalculusResult, CalculusValue},
        dispatch::{DomainRequest, DomainResult},
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

/// Run PlanIR steps with a single provider callback (invoked at most once).
///
/// Bootstrap semantics:
/// - `Normalize` / `SelectRepresentation` — acknowledged markers (no hidden policy).
/// - `CallDomainProvider` — invoke `provider` exactly once.
/// - `CrossDomainView` — open TypedView after provider output exists.
/// - `Verify` — recompute calculus/polynomial against claimed result; presence gate elsewhere.
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
    let verify_snapshot = VerifySnapshot::from_request(&request);
    let mut pending_request = Some(request);
    let mut provider = Some(provider);

    for step in &plan.steps {
        match *step {
            PlanStep::Normalize | PlanStep::SelectRepresentation => {
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
    use athena_types::{AssumptionSet, SymbolId, TermId};

    #[test]
    fn interpret_runs_normalize_verify_materialize() {
        let mut session = Session::new();
        let request = DomainRequest::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
            variable: SymbolId(0),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        });
        let plan = DomainPlan { steps: vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::Verify, PlanStep::MaterializeResult] };
        let (result, report) = interpret_domain_plan(&mut session, &plan, request, |s, r| crate::domains::dispatch::call_domain_provider(s, r))
            .expect("interpret");
        assert!(matches!(result, DomainResult::Calculus(_)));
        assert_eq!(report.executed, vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::Verify, PlanStep::MaterializeResult,]);
        assert!(report.provider_invoked && report.verified && report.materialized);
    }

    #[test]
    fn interpret_rejects_verify_before_provider() {
        let mut session = Session::new();
        let request = DomainRequest::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
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
        let request = DomainRequest::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
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
        let request = DomainRequest::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
            variable: SymbolId(0),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        });
        let plan = plan_domain(&request);
        let (_result, report) =
            interpret_domain_plan(&mut session, &plan, request, |s, r| crate::domains::dispatch::call_domain_provider(s, r))
                .expect("default plan");
        assert!(report.provider_invoked && report.verified && report.materialized);
    }
}
