//! 领域规划器 / [`DomainPlan`] 骨架。
//!
//! Goal 描述意图。算法 / 表示 / 后端选择落在此处——
//! 不要藏在领域 provider 里用隐式 `if len > …` 策略。
//!
//! Bootstrap 计划由 [`crate::domains::plan_exec::interpret_domain_plan`] 解释。
//! 默认形状：`Normalize` → `SelectRepresentation` → `CallDomainProvider` → `Verify` → `MaterializeResult`。
//! 级数族微积分目标在 provider 之后插入 `CrossDomainView`。

use crate::domains::{calculus::CalculusRequest, dispatch::DomainRequest};

/// [`DomainPlan`] 中的一步（规划原子 · 不是 IR 层）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStep {
    /// 将输入规范化 / 强制到所选表示。
    Normalize,
    /// 为涉及的 DomainObject 选择表示族。
    SelectRepresentation,
    /// 借用跨域 TypedView（禁止 `Vec` 拷贝）。
    CrossDomainView,
    /// 调用所属领域 provider / kernel。
    CallDomainProvider,
    /// 重放证书 / fingerprint（kernel 不得单独接纳事实）。
    Verify,
    /// 将 provider 输出投影为 [`crate::domains::DomainResult`]。
    MaterializeResult,
    /// 计划无法完成时发出未求值残差。
    EmitResidual,
}

/// 一次 [`DomainRequest`] 的计划执行。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct DomainPlan {
    /// 有序 [`PlanStep`]。
    pub steps: Vec<PlanStep>,
}

impl DomainPlan {
    /// 拥有式深复制（仅 `PlanStep` 句柄向量）。
    pub fn owning_copy(&self) -> Self {
        Self { steps: self.steps.clone() }
    }
}

/// 为 `request` 构建 [`DomainPlan`]（DomainPlanner 入口）。
///
/// 领域相关算法选择落在此处——不要在 `execute_*` 助手里做成静默策略分支。
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
