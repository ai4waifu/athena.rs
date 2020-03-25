//! ProofObligation · Reflection · SemanticReflector。
//!
//! 领域算法不是顶层真相源。它们通过 Reflector 观察 M-Graph 缺口并返回
//! `AlreadyKnown` / `Need*` / `Inconclusive`。`execute_domain` 只应出现在
//! `NeedComputation` 选出的内部 DomainPlan 中。

pub mod execute;
pub mod index;
pub mod schedule;

use crate::{
    domains::planner::DomainPlan,
    reasoning::mgraph::core::{
        MGraphView,
        refs::{ObjectRef, PredicateId, RelationRef, ScopeRef},
    },
};

pub use execute::{
    PlanBinding, QueuedPlan, QueuedPlanBatchReport, execute_queued_plan, plan_binding_for_request, run_next_queued_plan, run_queued_plans,
    verify_plan_binding,
};
pub use index::{ObligationIndex, ReflectorWake, WakeReport};
pub use schedule::{ReflectorScheduleReport, resume_reflector_frontier, schedule_reflector_wakes};

/// 待证明 / 待填补的语义缺口。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct ProofObligation {
    /// 闭合谓词（禁止字符串 label）。
    pub predicate: PredicateId,
    /// 作用域纤维。
    pub scope: ScopeRef,
    /// 部分已知参量（未绑定位置稍后扩展为 PartialArguments）。
    pub known_objects: Vec<ObjectRef>,
}

impl ProofObligation {
    /// Owning 复制（仅句柄向量）。
    pub fn owning_copy(&self) -> Self {
        Self { predicate: self.predicate, scope: self.scope, known_objects: self.known_objects.clone() }
    }
}

/// Reflector 对一次 obligation / query 的回应。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
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
    /// 需要执行 DomainPlan（，嵌于本分支）。
    NeedComputation {
        /// 已规划步骤。
        plan: DomainPlan,
    },
    /// 资源 / 搜索未完成，保留 frontier 身份（payload 稍后挂接）。
    Inconclusive,
}

impl Reflection {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::AlreadyKnown { relation } => Self::AlreadyKnown { relation: *relation },
            Self::NeedRelation { obligation } => Self::NeedRelation { obligation: obligation.owning_copy() },
            Self::NeedObject { object_kind } => Self::NeedObject { object_kind },
            Self::NeedConversion { source, target } => Self::NeedConversion { source, target },
            Self::NeedComputation { plan } => Self::NeedComputation { plan: plan.owning_copy() },
            Self::Inconclusive => Self::Inconclusive,
        }
    }
}

/// 语义 Reflector（缺口驱动）。
///
/// 与 [`crate::reasoning::solver::Reflector`]（旧调度侧 `ReflectionResult`）不同：
/// 本 trait 返回 [`Reflection`] 枚举，不得直接写 admitted relation。
pub trait SemanticReflector: Send + Sync {
    /// 观察 M-Graph 视图并报告缺口或已知事实。
    fn reflect(&self, obligation: &ProofObligation, view: &MGraphView<'_>) -> Reflection;
}
