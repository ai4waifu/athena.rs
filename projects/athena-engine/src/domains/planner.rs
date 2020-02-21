//! Domain planner / `DomainPlan` scaffold (Living `28`).
//!
//! Goal describes intent. Algorithm / representation / backend choices belong here —
//! not inside domain providers as hidden `if len > …` policy.
//!
//! Bootstrap plans are interpreted by [`crate::domains::plan_exec::interpret_domain_plan`].
//! Default shape: `Normalize` → `SelectRepresentation` → `CallDomainProvider` → `Verify` → `MaterializeResult`.
//! Series-family calculus goals insert `CrossDomainView` after the provider.

use crate::domains::{calculus::CalculusRequest, dispatch::DomainRequest};

/// One step in a [`DomainPlan`] (planner atom · not an IR layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStep {
    /// Normalize / coerce inputs to the selected representation.
    Normalize,
    /// Choose representation family for DomainObjects involved.
    SelectRepresentation,
    /// Borrow a cross-domain TypedView (no Vec copy).
    CrossDomainView,
    /// Invoke the owning domain provider / kernel.
    CallDomainProvider,
    /// Replay certificates / fingerprints (kernel must not admit facts alone).
    Verify,
    /// Project provider output into [`crate::domains::DomainResult`].
    MaterializeResult,
    /// Emit an unevaluated residual when the plan cannot complete.
    EmitResidual,
}

/// Planned execution for one [`DomainRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainPlan {
    /// Ordered [`PlanStep`]s.
    pub steps: Vec<PlanStep>,
}

/// Build a [`DomainPlan`] for `request` (Living `28` DomainPlanner entry).
///
/// Domain-specific algorithm selection lands here — not inside `execute_*` helpers
/// as silent strategy branches.
pub fn plan_domain(request: &DomainRequest) -> DomainPlan {
    match request {
        DomainRequest::Calculus(CalculusRequest::Series { .. } | CalculusRequest::Laurent { .. } | CalculusRequest::Asymptotic { .. }) => {
            DomainPlan {
                steps: vec![
                    PlanStep::Normalize,
                    PlanStep::SelectRepresentation,
                    PlanStep::CallDomainProvider,
                    PlanStep::CrossDomainView,
                    PlanStep::Verify,
                    PlanStep::MaterializeResult,
                ],
            }
        }
        _ => DomainPlan {
            steps: vec![
                PlanStep::Normalize,
                PlanStep::SelectRepresentation,
                PlanStep::CallDomainProvider,
                PlanStep::Verify,
                PlanStep::MaterializeResult,
            ],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::calculus::DerivativeOrder;
    use athena_types::{AssumptionSet, SymbolId, TermId};

    #[test]
    fn calculus_goal_gets_normalize_provider_verify_materialize() {
        let request = DomainRequest::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
            variable: SymbolId(0),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        });
        let plan = plan_domain(&request);
        assert_eq!(
            plan.steps,
            vec![
                PlanStep::Normalize,
                PlanStep::SelectRepresentation,
                PlanStep::CallDomainProvider,
                PlanStep::Verify,
                PlanStep::MaterializeResult,
            ]
        );
    }

    #[test]
    fn series_goal_inserts_cross_domain_view_step() {
        let request = DomainRequest::Calculus(CalculusRequest::Series {
            expression: TermId(0),
            variable: SymbolId(0),
            center: TermId(1),
            order: 2,
            assumptions: AssumptionSet::empty(),
        });
        let plan = plan_domain(&request);
        assert_eq!(
            plan.steps,
            vec![
                PlanStep::Normalize,
                PlanStep::SelectRepresentation,
                PlanStep::CallDomainProvider,
                PlanStep::CrossDomainView,
                PlanStep::Verify,
                PlanStep::MaterializeResult,
            ]
        );
    }
}
