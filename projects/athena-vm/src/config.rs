//! 执行配置（不含语义规划器状态）。

use athena_gc::GcMode;

use crate::{budget::StepBudget, cancel::CancellationToken};

/// VM 运行配置。
///
/// `GcMode` 只影响本执行作用域内的主动回收倾向，**不**改变回收权限归属。
#[derive(Debug, Clone, Default)]
pub struct VmConfig {
    /// 本执行有效的 GC 模式。
    pub gc_mode: GcMode,
    /// 共享解释步数预算（嵌套入口克隆同一 [`StepBudget`]）。
    pub step_budget: StepBudget,
    /// 协作式取消令牌。
    pub cancellation: CancellationToken,
}

impl VmConfig {
    /// 默认配置（延迟 GC · 无步数上限 · 未取消）。
    pub fn new() -> Self {
        Self { gc_mode: GcMode::Deferred, step_budget: StepBudget::unlimited(), cancellation: CancellationToken::new() }
    }

    /// 设置最大解释步数（安装独立 [`StepBudget::limited`]）。
    pub fn with_max_steps(mut self, max_steps: u64) -> Self {
        self.step_budget = StepBudget::limited(max_steps);
        self
    }

    /// 绑定共享步数预算（嵌套入口继承）。
    pub fn with_step_budget(mut self, budget: StepBudget) -> Self {
        self.step_budget = budget;
        self
    }

    /// 设置 GC 模式。
    pub fn with_gc_mode(mut self, gc_mode: GcMode) -> Self {
        self.gc_mode = gc_mode;
        self
    }

    /// 绑定取消令牌。
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = token;
        self
    }

    /// 兼容旧字段名：有限预算时的当前剩余，无上限为 `None`。
    pub fn max_steps(&self) -> Option<u64> {
        self.step_budget.remaining()
    }
}
