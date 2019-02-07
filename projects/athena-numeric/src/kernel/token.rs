//! Kernel 执行令牌：证明 pin / 容量 / 本次禁止 GC（不是完整 `NumericContext`）。

use core::marker::PhantomData;

/// 单次 machine-kernel 调用的前置证明。
///
/// 由 executor / `NumericContext` 在调用 `KernelTable` 前构造。Kernel **不得**持有
/// 完整 context，也不得在令牌存活期间触发 GC / 扩容 / 分配。
#[derive(Debug, Clone, Copy)]
pub struct ExecutionToken<'a> {
    _pin: PhantomData<&'a ()>,
}

impl<'a> ExecutionToken<'a> {
    /// 由上层证明：segment 已 pin（或本调用无 heap 视图）、输入合法、输出容量足够。
    #[inline]
    pub(crate) fn issue(_proof: KernelPreconditions) -> Self {
        Self { _pin: PhantomData }
    }

    /// 测试 / pure 路径：无 pin 需求时的轻量令牌（仍禁止 kernel 内分配）。
    #[inline]
    pub fn unverified_for_tests() -> Self {
        Self { _pin: PhantomData }
    }
}

/// Executor 在发令牌前检查的前置条件（不进入 kernel API 表面）。
#[derive(Debug, Clone, Copy)]
pub struct KernelPreconditions {
    /// 输出缓冲 limb 容量。
    pub out_capacity: usize,
    /// 所需输出上限（含进位）。
    pub out_need: usize,
}

impl KernelPreconditions {
    pub(crate) fn checked(out_capacity: usize, out_need: usize) -> Option<Self> {
        (out_capacity >= out_need.max(1)).then_some(Self { out_capacity, out_need })
    }
}
