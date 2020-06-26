//! 单次 VM 执行上下文：root set（经 [`ExecutionLease`]）+ safepoint 合同。
//!
//! Living `04`：VM 必须接收执行上下文与 root set，并在 safepoint 参与 GC 合同。
//! 禁止在解释循环内猜测长期对象存活，也禁止把 lease 做成第二套 GC。

use athena_gc::GcMode;
use athena_types::Result;

use crate::lease::ExecutionLease;

/// 一次 `VmExecutor::execute*` 的可变执行上下文。
#[derive(Debug)]
pub struct VmExecutionContext<'a> {
    lease: Option<&'a mut ExecutionLease>,
    /// 本执行进入 safepoint 的次数（显式 `Safepoint` 指令或隐式宿主 safepoint）。
    pub safepoint_count: u64,
    /// 本执行在 safepoint 触发的 `collect` 次数。
    pub collect_count: u64,
}

impl<'a> VmExecutionContext<'a> {
    /// 携带执行期 lease / root 集合。
    pub fn with_lease(lease: &'a mut ExecutionLease) -> Self {
        Self { lease: Some(lease), safepoint_count: 0, collect_count: 0 }
    }

    /// 无 lease 的空上下文（骨架 / 不参与 GC 的冒烟路径）。
    pub fn detached() -> Self {
        Self { lease: None, safepoint_count: 0, collect_count: 0 }
    }

    /// 当前是否挂着执行 lease。
    #[inline]
    pub fn has_lease(&self) -> bool {
        self.lease.is_some()
    }

    /// 只读访问 lease（若有）。
    #[inline]
    pub fn lease(&self) -> Option<&ExecutionLease> {
        self.lease.as_deref()
    }

    /// 可变访问 lease（若有）。
    #[inline]
    pub fn lease_mut(&mut self) -> Option<&mut ExecutionLease> {
        self.lease.as_deref_mut()
    }

    /// 进入 GC / 取消协作 safepoint。
    ///
    /// 有 lease 时委托 [`ExecutionLease::enter_safepoint`]；无 lease 时只记账。
    pub fn enter_safepoint(&mut self, mode: GcMode) -> Result<()> {
        self.safepoint_count = self.safepoint_count.saturating_add(1);
        if let Some(lease) = self.lease.as_mut() {
            if lease.enter_safepoint(mode)? {
                self.collect_count = self.collect_count.saturating_add(1);
            }
        }
        Ok(())
    }
}
