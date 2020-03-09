//! M-Graph 状态容器。

use crate::reasoning::mgraph::{admission::semantic::SemanticCore, closure::operational::OperationalState};

/// M-Graph 状态：semantic core 与 operational 层分离。
///
/// Living `31`：**不**实现 [`Clone`]（operational 缓存含多项式 owning 载荷）。
#[derive(Debug, Default, PartialEq)]
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
        store: &athena_ir::TermStore,
        limits: &crate::reasoning::mgraph::ClosureLimits,
    ) -> crate::reasoning::mgraph::ClosureResult {
        crate::reasoning::mgraph::run_closure_step(store, self, limits)
    }
}
