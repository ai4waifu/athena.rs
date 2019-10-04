//! Typed domain goals — never construct dialect-shaped applications.

use athena_engine::api::DomainGoal;
use athena_engine::domains::DomainRequest;
use athena_engine::domains::calculus::{CalculusRequest, DerivativeOrder, LimitApproach, LimitDirection};
use athena_types::{AssumptionSet, SymbolId, TermId};

/// Constructs [`DomainGoal`] values from typed calculus / domain requests.
#[derive(Debug, Default, Clone, Copy)]
pub struct DomainRequestBuilder;

impl DomainRequestBuilder {
    /// `CalculusRequest::Derivative` wrapped as [`DomainGoal`].
    pub fn derivative(
        self,
        expression: TermId,
        variable: SymbolId,
        order: DerivativeOrder,
        assumptions: AssumptionSet,
    ) -> DomainGoal {
        DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Derivative {
            expression,
            variable,
            order,
            assumptions,
        }))
    }

    /// First-order derivative convenience.
    pub fn derivative_first(self, expression: TermId, variable: SymbolId) -> DomainGoal {
        self.derivative(expression, variable, DerivativeOrder::First, AssumptionSet::empty())
    }

    /// Indefinite integral.
    pub fn integral(self, expression: TermId, variable: SymbolId) -> DomainGoal {
        DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Integral {
            expression,
            variable,
            assumptions: AssumptionSet::empty(),
        }))
    }

    /// Limit.
    pub fn limit(
        self,
        expression: TermId,
        variable: SymbolId,
        approach: LimitApproach,
        direction: LimitDirection,
    ) -> DomainGoal {
        DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Limit {
            expression,
            variable,
            approach,
            direction,
            assumptions: AssumptionSet::empty(),
        }))
    }

    /// Wrap an existing [`DomainRequest`].
    pub fn dispatch(self, request: DomainRequest) -> DomainGoal {
        DomainGoal::Dispatch(request)
    }
}
