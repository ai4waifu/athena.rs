//! 执行配置（不含语义 planner 状态）。

use athena_gc::GcMode;

/// VM 运行配置。
///
/// `GcMode` 只影响本执行作用域内的主动 collect 倾向，**不**改变 reclaim authority。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmConfig {
    /// 本执行有效的 GC 模式。
    pub gc_mode: GcMode,
    /// 最大解释步数（`None` = 不设 VM 层步数上限）。
    pub max_steps: Option<u64>,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            gc_mode: GcMode::Deferred,
            max_steps: None,
        }
    }
}

impl VmConfig {
    /// 默认配置（Deferred GC · 无步数上限）。
    pub const fn new() -> Self {
        Self {
            gc_mode: GcMode::Deferred,
            max_steps: None,
        }
    }

    /// 设置最大解释步数。
    pub const fn with_max_steps(mut self, max_steps: u64) -> Self {
        self.max_steps = Some(max_steps);
        self
    }

    /// 设置 GC 模式。
    pub const fn with_gc_mode(mut self, gc_mode: GcMode) -> Self {
        self.gc_mode = gc_mode;
        self
    }
}
