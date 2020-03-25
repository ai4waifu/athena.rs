//! 调度 Reflector 唤醒与 frontier 续跑（· bootstrap）。

use crate::reasoning::mgraph::{
    core::state::MGraphState,
    obligation::{ProofObligation, QueuedPlan, Reflection, ReflectorWake, SemanticReflector},
};

/// 将 Reflector 结果写入运行态队列后的计数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReflectorScheduleReport {
    /// 解析为 [`Reflection::AlreadyKnown`] 的唤醒数。
    pub already_known: u32,
    /// 已入队等待域执行的计划数。
    pub need_computation: u32,
    /// 重新登记进义务索引的嵌套义务数。
    pub need_relation: u32,
    /// 对象缺口（仅计数；bootstrap 不入队）。
    pub need_object: u32,
    /// 换算缺口（仅计数；bootstrap 不入队）。
    pub need_conversion: u32,
    /// 推入续跑队列、稍后再次反射的义务数。
    pub inconclusive_resumed: u32,
}

/// 对一批唤醒应用 Reflector 结果并写入运行态队列。
///
/// **不**接纳事实，也 **不**调用 `execute_domain`。
pub fn schedule_reflector_wakes(
    state: &mut MGraphState,
    wakes: &[ReflectorWake],
    reflector: &dyn SemanticReflector,
) -> ReflectorScheduleReport {
    let outcomes: Vec<Reflection> = {
        let view = state.semantic.view();
        wakes.iter().map(|wake| reflector.reflect(&wake.obligation, &view)).collect()
    };
    apply_reflections(state, wakes.iter().map(|w| &w.obligation), outcomes)
}

/// 从续跑队列取出义务并再次反射（frontier resume）。
pub fn resume_reflector_frontier(state: &mut MGraphState, reflector: &dyn SemanticReflector) -> ReflectorScheduleReport {
    let pending = std::mem::take(&mut state.operational.resume_queue);
    let outcomes: Vec<Reflection> = {
        let view = state.semantic.view();
        pending.iter().map(|obligation| reflector.reflect(obligation, &view)).collect()
    };
    apply_reflections(state, pending.iter(), outcomes)
}

fn apply_reflections<'a>(
    state: &mut MGraphState,
    obligations: impl Iterator<Item = &'a ProofObligation>,
    outcomes: Vec<Reflection>,
) -> ReflectorScheduleReport {
    let mut report = ReflectorScheduleReport::default();
    for (obligation, outcome) in obligations.zip(outcomes) {
        match outcome {
            Reflection::AlreadyKnown { .. } => {
                report.already_known = report.already_known.saturating_add(1);
            }
            Reflection::NeedComputation { plan } => {
                // 唤醒路径尚无 DomainRequest — 指纹在执行时再绑定。
                state.operational.pending_plans.push(QueuedPlan::unbound(plan, obligation.owning_copy()));
                report.need_computation = report.need_computation.saturating_add(1);
            }
            Reflection::NeedRelation { obligation: nested } => {
                state.operational.obligation_index.register(nested);
                report.need_relation = report.need_relation.saturating_add(1);
            }
            Reflection::NeedObject { .. } => {
                report.need_object = report.need_object.saturating_add(1);
            }
            Reflection::NeedConversion { .. } => {
                report.need_conversion = report.need_conversion.saturating_add(1);
            }
            Reflection::Inconclusive => {
                state.operational.resume_queue.push(obligation.owning_copy());
                report.inconclusive_resumed = report.inconclusive_resumed.saturating_add(1);
            }
        }
    }
    report
}
