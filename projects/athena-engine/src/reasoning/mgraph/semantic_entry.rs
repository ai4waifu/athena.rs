//! Living `29` 顶层语义入口：Goal → Obligation → Reflector → Plan / Result。
//!
//! `execute_domain` 只应出现在本模块的 `NeedComputation` 分支内，不得冒充顶层语义路径。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    api::request::DomainGoal,
    domains::{
        calculus::CalculusRequest,
        dispatch::{DomainRequest, DomainResult, execute_domain},
        planner::{PlanStep, plan_domain},
    },
    reasoning::mgraph::{
        CalculusReflector, MGraphCore, MGraphView, ObjectRef, PolynomialReflector, ProofObligation, Reflection, RelationRef,
        ScopeRef, SemanticReflector, TheoryContextId, predicates,
    },
    runtime::session::Session,
};

/// 语义入口对一次 [`DomainGoal`] 的结果（Living `29`）。
#[derive(Debug, PartialEq)]
pub enum DomainSemanticOutcome {
    /// M-Graph 已有足够强的 admitted relation。
    AlreadyKnown {
        /// 已接纳关系。
        relation: RelationRef,
    },
    /// Reflector 选出 DomainPlan 后经 provider 算出的结果。
    Computed(DomainResult),
    /// 缺少领域对象（须构造 DomainObject / lowering）。
    NeedObject {
        /// 机器标识（非前端名）。
        object_kind: &'static str,
    },
    /// 缺少显式域映射。
    NeedConversion {
        /// 源域标签。
        source: &'static str,
        /// 目标域标签。
        target: &'static str,
    },
    /// 资源 / 搜索未完成。
    Inconclusive,
}

/// 从 [`DomainRequest`] 构造缺口义务（无则返回 `None`，走通用 NeedComputation）。
pub fn obligation_from_domain_request(request: &DomainRequest) -> Option<ProofObligation> {
    match request {
        DomainRequest::Calculus(CalculusRequest::Derivative { expression, variable, .. }) => Some(ProofObligation {
            predicate: predicates::DERIVATIVE_OF,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(expression.0)),
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(variable.0)),
            ],
        }),
        DomainRequest::Calculus(CalculusRequest::Integral { expression, variable, .. })
        | DomainRequest::Calculus(CalculusRequest::DefiniteIntegral { expression, variable, .. }) => Some(ProofObligation {
            predicate: predicates::INTEGRAL_OF,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(expression.0)),
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(variable.0)),
            ],
        }),
        DomainRequest::Calculus(CalculusRequest::Series { expression, variable, .. })
        | DomainRequest::Calculus(CalculusRequest::Laurent { expression, variable, .. })
        | DomainRequest::Calculus(CalculusRequest::Asymptotic { expression, variable, .. }) => Some(ProofObligation {
            predicate: predicates::SERIES_EXPANSION,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(expression.0)),
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(variable.0)),
            ],
        }),
        DomainRequest::Polynomial(_) => Some(ProofObligation {
            predicate: predicates::POLYNOMIAL_RESULT,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: Vec::new(),
        }),
        _ => None,
    }
}

fn reflect_domain(request: &DomainRequest, obligation: &ProofObligation, view: &MGraphView<'_>) -> Reflection {
    match request {
        DomainRequest::Calculus(_) => CalculusReflector.reflect(obligation, view),
        DomainRequest::Polynomial(_) => PolynomialReflector.reflect(obligation, view),
        _ => Reflection::NeedComputation {
            plan: plan_domain(request),
        },
    }
}

/// Living `29` 语义入口：先查 M-Graph，再允许 `NeedComputation` → Living 28 Plan → provider。
pub fn execute_domain_goal(
    session: &mut Session,
    core: &MGraphCore,
    goal: DomainGoal,
) -> Result<DomainSemanticOutcome, Diagnostic> {
    let DomainGoal::Dispatch(request) = goal;
    let view = MGraphView::new(core);
    let reflection = match obligation_from_domain_request(&request) {
        Some(obligation) => reflect_domain(&request, &obligation, &view),
        None => Reflection::NeedComputation {
            plan: plan_domain(&request),
        },
    };
    match reflection {
        Reflection::AlreadyKnown { relation } => Ok(DomainSemanticOutcome::AlreadyKnown { relation }),
        Reflection::NeedObject { object_kind } => Ok(DomainSemanticOutcome::NeedObject { object_kind }),
        Reflection::NeedConversion { source, target } => Ok(DomainSemanticOutcome::NeedConversion { source, target }),
        Reflection::NeedRelation { .. } => Ok(DomainSemanticOutcome::Inconclusive),
        Reflection::Inconclusive => Ok(DomainSemanticOutcome::Inconclusive),
        Reflection::NeedComputation { plan } => {
            if !plan.steps.iter().any(|s| matches!(s, PlanStep::CallDomainProvider)) {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "semantic_entry")
                    .detail("reason", "plan missing CallDomainProvider"));
            }
            let result = execute_domain(session, request)?;
            Ok(DomainSemanticOutcome::Computed(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::calculus::{CalculusRequest, DerivativeOrder};
    use athena_types::{AssumptionSet, SymbolId, TermId};

    #[test]
    fn calculus_goal_computes_when_graph_empty() {
        let mut session = Session::new();
        let core = MGraphCore::new();
        let goal = DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
            variable: SymbolId(0),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        }));
        match execute_domain_goal(&mut session, &core, goal).expect("ok") {
            DomainSemanticOutcome::Computed(DomainResult::Calculus(_)) => {}
            other => panic!("expected Computed calculus, got {other:?}"),
        }
    }

    #[test]
    fn polynomial_goal_needs_object_when_empty() {
        use crate::domains::polynomial::{Polynomial, PolynomialRequest};
        let mut session = Session::new();
        let core = MGraphCore::new();
        let goal = DomainGoal::Dispatch(DomainRequest::Polynomial(PolynomialRequest::Normalize {
            polynomial: Polynomial::zero(athena_types::RingId(0)),
        }));
        match execute_domain_goal(&mut session, &core, goal).expect("ok") {
            DomainSemanticOutcome::NeedObject { object_kind } => assert_eq!(object_kind, "PolynomialRef"),
            other => panic!("expected NeedObject, got {other:?}"),
        }
    }
}
