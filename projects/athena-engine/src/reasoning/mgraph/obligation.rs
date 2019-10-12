//! ProofObligation · Reflection · SemanticReflector（Living `29`）。
//!
//! 领域算法不是顶层真相源。它们通过 Reflector 观察 M-Graph 缺口并返回
//! `AlreadyKnown` / `Need*` / `Inconclusive`。`execute_domain` 只应出现在
//! `NeedComputation` 选出的内部 DomainPlan 中。

use crate::{
    domains::planner::DomainPlan,
    reasoning::mgraph::core::{
        refs::{ObjectRef, PredicateId, RelationRef, ScopeRef},
        MGraphView,
    },
};

/// 待证明 / 待填补的语义缺口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofObligation {
    /// 闭合谓词（禁止字符串 label）。
    pub predicate: PredicateId,
    /// 作用域纤维。
    pub scope: ScopeRef,
    /// 部分已知参量（未绑定位置稍后扩展为 PartialArguments）。
    pub known_objects: Vec<ObjectRef>,
}

/// Reflector 对一次 obligation / query 的回应（Living `29`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reflection {
    /// M-Graph 中已有足够强的 admitted relation。
    AlreadyKnown {
        /// 已接纳关系。
        relation: RelationRef,
    },
    /// 缺少一条可验证关系（继续 Reflector / CapabilityPlanner）。
    NeedRelation {
        /// 嵌套义务。
        obligation: ProofObligation,
    },
    /// 缺少领域对象（构造 DomainObject / lowering）。
    NeedObject {
        /// 人类可读缺口标签（机器标识，非前端名）。
        object_kind: &'static str,
    },
    /// 缺少显式域映射 / coercion。
    NeedConversion {
        /// 源域标签。
        source: &'static str,
        /// 目标域标签。
        target: &'static str,
    },
    /// 需要执行 DomainPlan（Living `28`，嵌于本分支）。
    NeedComputation {
        /// 已规划步骤。
        plan: DomainPlan,
    },
    /// 资源 / 搜索未完成，保留 frontier 身份（payload 稍后挂接）。
    Inconclusive,
}

/// Living `29` 语义 Reflector（缺口驱动）。
///
/// 与 [`crate::reasoning::solver::Reflector`]（旧调度侧 `ReflectionResult`）不同：
/// 本 trait 返回 [`Reflection`] 枚举，不得直接写 admitted relation。
pub trait SemanticReflector: Send + Sync {
    /// 观察 M-Graph 视图并报告缺口或已知事实。
    fn reflect(&self, obligation: &ProofObligation, view: &MGraphView<'_>) -> Reflection;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domains::planner::{PlanStep, plan_domain},
        domains::DomainRequest,
        domains::calculus::{CalculusRequest, DerivativeOrder},
        reasoning::mgraph::core::MGraphCore,
    };
    use athena_types::{AssumptionSet, SymbolId, TermId};

    struct AlwaysCompute;

    impl SemanticReflector for AlwaysCompute {
        fn reflect(&self, _obligation: &ProofObligation, _view: &MGraphView<'_>) -> Reflection {
            let request = DomainRequest::Calculus(CalculusRequest::Derivative {
                expression: TermId(0),
                variable: SymbolId(0),
                order: DerivativeOrder::First,
                assumptions: AssumptionSet::empty(),
            });
            Reflection::NeedComputation {
                plan: plan_domain(&request),
            }
        }
    }

    #[test]
    fn need_computation_carries_living28_plan() {
        let core = MGraphCore::new();
        let view = MGraphView::new(&core);
        let obligation = ProofObligation {
            predicate: PredicateId(0),
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: Vec::new(),
        };
        match AlwaysCompute.reflect(&obligation, &view) {
            Reflection::NeedComputation { plan } => {
                assert_eq!(plan.steps, vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult]);
            }
            other => panic!("expected NeedComputation, got {other:?}"),
        }
    }
}
