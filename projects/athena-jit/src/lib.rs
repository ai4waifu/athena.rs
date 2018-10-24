//! 可选 JIT 插件（非核心语义层）。默认不可用，WASM 须报告 `UnsupportedTarget`。
#![deny(missing_docs)]

/// JIT 可用性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitAvailability {
    /// Feature 未启用。
    Disabled,
    /// 目标平台不支持（含 wasm32）。
    UnsupportedTarget,
    /// 本地编译器可用（内核未接时为预留态）。
    Available,
}

/// 查询 JIT 可用性（当前无 native 内核时为 [`JitAvailability::UnsupportedTarget`]）。
pub fn availability() -> JitAvailability {
    #[cfg(target_arch = "wasm32")]
    {
        JitAvailability::UnsupportedTarget
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        JitAvailability::UnsupportedTarget
    }
}

/// Parity 对比结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParityOutcome {
    /// 仅 eager。
    EagerOnly,
    /// JIT 不可用。
    JitUnavailable,
    /// 一致。
    Matched,
    /// 不一致。
    Mismatch {
        /// 摘要。
        detail: String,
    },
}

/// 对比 eager 与 JIT 回调结果（JIT 返回 `None` 表示不可用）。
pub fn polynomial_mul_parity<P: PartialEq>(eager: impl FnOnce() -> P, jit: impl FnOnce() -> Option<P>) -> ParityOutcome {
    let e = eager();
    match jit() {
        None => ParityOutcome::JitUnavailable,
        Some(j) if j == e => ParityOutcome::Matched,
        Some(_) => ParityOutcome::Mismatch { detail: "polynomial_mul".into() },
    }
}
