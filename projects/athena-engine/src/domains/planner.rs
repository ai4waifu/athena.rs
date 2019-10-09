//! Domain planner / PlanIR scaffold (Living `28`).
//!
//! Goal describes intent. Algorithm / representation / backend choices belong here —
//! not inside domain providers as hidden `if len > …` policy.
//!
//! Bootstrap: every [`DomainRequest`] plans as
//! `CallDomainProvider` → `MaterializeResult`. Later slices add normalize,
//! representation selection, TypedView, Verify, and capability budgets.

use crate::domains::dispatch::DomainRequest;

/// One step in a domain execution plan (PlanIR atom).
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
    /// Ordered PlanIR steps.
    pub steps: Vec<PlanStep>,
}

/// Build a [`DomainPlan`] for `request` (Living `28` DomainPlanner entry).
///
/// Bootstrap policy is uniform: call the domain provider then materialize.
/// Domain-specific algorithm selection lands here in later slices — not inside
/// `execute_*` helpers as silent strategy branches.
pub fn plan_domain(_request: &DomainRequest) -> DomainPlan {
    DomainPlan {
        steps: vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::calculus::{CalculusRequest, DerivativeOrder};
    use athena_types::{AssumptionSet, SymbolId, TermId};

    #[test]
    fn calculus_goal_gets_provider_then_materialize_plan() {
        let request = DomainRequest::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
            variable: SymbolId(0),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        });
        let plan = plan_domain(&request);
        assert_eq!(
            plan.steps,
            vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult]
        );
    }
}
