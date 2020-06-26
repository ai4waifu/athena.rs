//! 执行配置（不含语义规划器状态）。

use athena_gc::GcMode;

use crate::cancel::CancellationToken;

/// VM 运行配置。
///
/// `GcMode` 只影响本执行作用域内的主动回收倾向，**不**改变回收权限归属。
#[derive(Debug, Clone, Default)]
pub struct VmConfig {
    /// 本执行有效的 GC 模式。
    pub gc_mode: GcMode,
    /// 最大解释步数（`None` 表示不设 VM 层步数上限）。
    pub max_steps: Option<u64>,
    /// 协作式取消令牌。
    pub cancellation: CancellationToken,
}

impl VmConfig {
    /// 默认配置（延迟 GC · 无步数上限 · 未取消）。
    pub fn new() -> Self {
        Self { gc_mode: GcMode::Deferred, max_steps: None, cancellation: CancellationToken::new() }
    }

    /// 设置最大解释步数。
    pub fn with_max_steps(mut self, max_steps: u64) -> Self {
        self.max_steps = Some(max_steps);
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
}
