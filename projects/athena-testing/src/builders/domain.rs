//! 类型化领域目标 — 绝不构造方言形态的应用。

use athena_engine::{
    api::DomainGoal,
    domains::{
        DomainRequest,
        calculus::{CalculusRequest, DerivativeOrder, LimitApproach, LimitDirection},
    },
};
use athena_types::{AssumptionSet, SymbolId, TermId};

/// 由类型化微积分 / 领域请求构造 [`DomainGoal`]。
#[derive(Debug, Default, Clone, Copy)]
pub struct DomainRequestBuilder;

impl DomainRequestBuilder {
    /// 将 `CalculusRequest::Derivative` 包装为 [`DomainGoal`]。
    pub fn derivative(self, expression: TermId, variable: SymbolId, order: DerivativeOrder, assumptions: AssumptionSet) -> DomainGoal {
        DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Derivative { expression, variable, order, assumptions }))
    }

    /// 一阶导数便捷构造。
    pub fn derivative_first(self, expression: TermId, variable: SymbolId) -> DomainGoal {
        self.derivative(expression, variable, DerivativeOrder::First, AssumptionSet::empty())
    }

    /// 不定积分。
    pub fn integral(self, expression: TermId, variable: SymbolId) -> DomainGoal {
        DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Integral { expression, variable, assumptions: AssumptionSet::empty() }))
    }

    /// Limit.
    pub fn limit(self, expression: TermId, variable: SymbolId, approach: LimitApproach, direction: LimitDirection) -> DomainGoal {
        DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Limit {
            expression,
            variable,
            approach,
            direction,
            assumptions: AssumptionSet::empty(),
        }))
    }

    /// 包装已有 [`DomainRequest`]。
    pub fn dispatch(self, request: DomainRequest) -> DomainGoal {
        DomainGoal::Dispatch(request)
    }
}
