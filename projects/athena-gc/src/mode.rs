//! `GcMode` 与作用域 guard（禁止进程级全局开关）。
#![allow(unsafe_code)]

use core::{cell::Cell, ptr::NonNull};

use crate::ids::SegmentId;

/// GC 收集策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GcMode {
    /// 达阈值时 safepoint + tracing + reclaim。
    #[default]
    Auto,
    /// 允许分配，不主动收集；记 pressure；显式 / 结束时 `collect`。
    Deferred,
    /// 不 tracing、不 reclaim、不 LRU eviction（仍强制 budget）。
    Disabled,
}

/// 堆压力快照（Deferred 下累计）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GcPressure {
    /// 自上次 collect 以来的分配字节。
    pub bytes_since_collect: usize,
    /// 分配次数。
    pub allocation_count: u64,
    /// 是否曾触及阈值。
    pub threshold_hit: bool,
}

/// 控制器状态（嵌入 `GcHeap`，经 `Rc` 共享给 guard）。
#[derive(Debug)]
pub struct GcController {
    mode: Cell<GcMode>,
    /// Suspend/Defer 嵌套深度（>0 时覆盖为 Disabled/Deferred）。
    suspend_depth: Cell<u32>,
    defer_depth: Cell<u32>,
    pressure: Cell<GcPressure>,
    /// Auto 触发阈值（字节）。
    auto_threshold_bytes: Cell<usize>,
}

impl Default for GcController {
    fn default() -> Self {
        Self {
            mode: Cell::new(GcMode::Auto),
            suspend_depth: Cell::new(0),
            defer_depth: Cell::new(0),
            pressure: Cell::new(GcPressure::default()),
            auto_threshold_bytes: Cell::new(16 * 1024 * 1024),
        }
    }
}

impl GcController {
    /// 新建（默认 Auto）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 配置的基准 mode（忽略临时 guard）。
    pub fn base_mode(&self) -> GcMode {
        self.mode.get()
    }

    /// 设置基准 mode（不影响已存在的 suspend/defer 深度）。
    pub fn set_base_mode(&self, mode: GcMode) {
        self.mode.set(mode);
    }

    /// 当前有效 mode（guard 覆盖 base）。
    pub fn effective_mode(&self) -> GcMode {
        if self.suspend_depth.get() > 0 {
            return GcMode::Disabled;
        }
        if self.defer_depth.get() > 0 {
            return GcMode::Deferred;
        }
        self.mode.get()
    }

    /// Auto 阈值字节。
    pub fn auto_threshold_bytes(&self) -> usize {
        self.auto_threshold_bytes.get()
    }

    /// 设置 Auto 阈值。
    pub fn set_auto_threshold_bytes(&self, bytes: usize) {
        self.auto_threshold_bytes.set(bytes.max(1));
    }

    /// 当前压力。
    pub fn pressure(&self) -> GcPressure {
        self.pressure.get()
    }

    /// 记录一次分配（Deferred/Disabled 也累计，供报告）。
    pub fn record_allocation(&self, bytes: usize) {
        let mut p = self.pressure.get();
        p.bytes_since_collect = p.bytes_since_collect.saturating_add(bytes);
        p.allocation_count = p.allocation_count.saturating_add(1);
        if p.bytes_since_collect >= self.auto_threshold_bytes.get() {
            p.threshold_hit = true;
        }
        self.pressure.set(p);
    }

    /// 清除压力（collect 后）。
    pub fn clear_pressure(&self) {
        self.pressure.set(GcPressure::default());
    }

    /// 是否应在 Auto 下尝试 collect。
    pub fn should_collect_after_alloc(&self) -> bool {
        matches!(self.effective_mode(), GcMode::Auto) && self.pressure.get().threshold_hit
    }

    /// 进入 Disabled（作用域）。
    pub fn suspend(self: &std::rc::Rc<Self>) -> GcSuspendGuard {
        self.suspend_depth.set(self.suspend_depth.get().saturating_add(1));
        GcSuspendGuard { ctrl: std::rc::Rc::clone(self) }
    }

    /// 进入 Deferred（作用域）。
    pub fn defer(self: &std::rc::Rc<Self>) -> GcDeferGuard {
        self.defer_depth.set(self.defer_depth.get().saturating_add(1));
        GcDeferGuard { ctrl: std::rc::Rc::clone(self) }
    }

    fn end_suspend(&self) {
        self.suspend_depth.set(self.suspend_depth.get().saturating_sub(1));
    }

    fn end_defer(&self) {
        self.defer_depth.set(self.defer_depth.get().saturating_sub(1));
    }
}

/// `GcMode::Disabled` 作用域 guard。
#[derive(Debug)]
pub struct GcSuspendGuard {
    ctrl: std::rc::Rc<GcController>,
}

impl Drop for GcSuspendGuard {
    fn drop(&mut self) {
        self.ctrl.end_suspend();
    }
}

/// `GcMode::Deferred` 作用域 guard。
#[derive(Debug)]
pub struct GcDeferGuard {
    ctrl: std::rc::Rc<GcController>,
}

impl Drop for GcDeferGuard {
    fn drop(&mut self) {
        self.ctrl.end_defer();
    }
}

/// Segment pin guard（持有期内禁止 reclaim 该段）。
///
/// 使用 `NonNull` 避免冻结 `GcHeap` 借用，便于 kernel 路径同时 `&mut` 写 limb。
pub struct GcPinGuard {
    heap: NonNull<crate::heap::GcHeap>,
    segments: Vec<SegmentId>,
}

impl GcPinGuard {
    pub(crate) fn new(heap: &crate::heap::GcHeap, segments: Vec<SegmentId>) -> Self {
        for id in &segments {
            heap.pin_segment(*id);
        }
        Self { heap: NonNull::from(heap), segments }
    }
}

impl Drop for GcPinGuard {
    fn drop(&mut self) {
        // SAFETY: heap 指针在 guard 存活期内有效（调用方合同：不 move/drop heap）。
        let heap = unsafe { self.heap.as_ref() };
        for id in &self.segments {
            heap.unpin_segment(*id);
        }
    }
}
