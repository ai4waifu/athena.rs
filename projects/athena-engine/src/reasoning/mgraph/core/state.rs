//! M-Graph 状态容器。

use crate::reasoning::mgraph::{admission::semantic::SemanticCore, closure::operational::OperationalState};

/// M-Graph 状态：semantic core 与 operational 层分离。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MGraphState {
    /// 单调 verified claims + 派生索引。
    pub semantic: SemanticCore,
    /// frontier · 非语义缓存 · 暂存结构。
    pub operational: OperationalState,
}

impl MGraphState {
    /// 空状态。
    pub fn new() -> Self {
        Self::default()
    }

    /// Run equality-forest closure in place (Living `26` / `29` bootstrap).
    pub fn run_closure(
        &mut self,
        limits: &crate::reasoning::mgraph::ClosureLimits,
    ) -> crate::reasoning::mgraph::ClosureResult {
        crate::reasoning::mgraph::run_closure_step(self, limits)
    }
}
