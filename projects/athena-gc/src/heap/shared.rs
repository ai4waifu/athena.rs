//! 构造 / 共享默认堆 / 访问器 / `Drop` / `heap_id_for_limbs`。

use core::{cell::Cell, ptr::NonNull};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

use crate::{
    batch::AllocationAccounting,
    budget::HeapBudget,
    header::AllocationHeader,
    ids::{HeapId, SegmentId},
    mode::{GcController, GcDeferGuard, GcMode, GcPinGuard, GcSuspendGuard},
    registry,
    root::RootRegistry,
    scratch::ScratchArena,
    stats::HeapStats,
};

use super::state::GcHeap;

impl GcHeap {
    /// 构造未登记的堆（须再包进 `Rc` 并 [`Self::into_shared`]）。
    fn new_inner(budget: HeapBudget) -> Self {
        Self {
            id: HeapId(0),
            budget,
            segments: Vec::new(),
            free_segment_slots: Vec::new(),
            next_generation: 1,
            access_clock: 0,
            resident_bytes: 0,
            controller: Rc::new(GcController::new()),
            roots: RootRegistry::new(),
            scratch: ScratchArena::new(),
            stats: HeapStats::default(),
            drop_busy_leaks: Rc::new(Cell::new(0)),
            objects: Vec::new(),
            free_objects: Vec::new(),
            traced_numeric: HashSet::new(),
            bump_ephemeral: false,
            accounting: AllocationAccounting::Full,
            batch_bytes: 0,
            batch_allocs: 0,
            registered: false,
        }
    }

    /// 共享堆（登记 `HeapId`，供 numeric Drop 回找）。
    pub fn new_shared(budget: HeapBudget) -> Rc<RefCell<Self>> {
        let rc = Rc::new(RefCell::new(Self::new_inner(budget)));
        let leaks = rc.borrow().drop_busy_leaks.clone();
        let id = registry::register(&rc, leaks);
        rc.borrow_mut().id = id;
        rc.borrow_mut().registered = true;
        rc
    }

    /// 兼容旧测试名：等价于 [`Self::new_shared`]。
    pub fn new(budget: HeapBudget) -> Rc<RefCell<Self>> {
        Self::new_shared(budget)
    }

    /// 线程默认共享堆（无显式 Session 时的 numeric 回退）。
    ///
    /// TLS 析构后不可再取；调用方应在正常执行路径使用，勿在全局 Drop 里依赖。
    pub fn shared_default() -> Rc<RefCell<Self>> {
        thread_local! {
            static DEFAULT: RefCell<Option<Rc<RefCell<GcHeap>>>> = const { RefCell::new(None) };
        }
        DEFAULT
            .try_with(|slot| {
                let mut guard = slot.borrow_mut();
                if guard.is_none() {
                    *guard = Some(Self::new_shared(HeapBudget::default()));
                }
                guard.as_ref().expect("default heap").clone()
            })
            .expect("shared_default while TLS alive")
    }

    /// 本堆 id。
    pub fn id(&self) -> HeapId {
        self.id
    }

    /// 预算。
    pub fn budget(&self) -> &HeapBudget {
        &self.budget
    }

    /// GC 控制器。
    pub fn gc(&self) -> &Rc<GcController> {
        &self.controller
    }

    /// 根注册表。
    pub fn roots(&self) -> &RootRegistry {
        &self.roots
    }

    /// 根注册表（可变）。
    pub fn roots_mut(&mut self) -> &mut RootRegistry {
        &mut self.roots
    }

    /// 暂存区。
    pub fn scratch(&mut self) -> &mut ScratchArena {
        &mut self.scratch
    }

    /// 统计信息。
    pub fn stats(&self) -> HeapStats {
        let mut s = self.stats;
        s.peak_scratch_bytes = s.peak_scratch_bytes.max(self.scratch.peak_bytes());
        s.drop_busy_leaks = self.drop_busy_leaks.get();
        s
    }

    /// 当前有效 [`GcMode`]。
    pub fn effective_mode(&self) -> GcMode {
        self.controller.effective_mode()
    }

    /// 挂起自动回收。
    pub fn suspend(&self) -> GcSuspendGuard {
        self.controller.suspend()
    }

    /// 推迟回收到守卫结束。
    pub fn defer(&self) -> GcDeferGuard {
        self.controller.defer()
    }

    /// 钉住给定 segment，回收时跳过。
    pub fn pin(&self, segments: &[SegmentId]) -> GcPinGuard {
        GcPinGuard::new(self, segments.to_vec())
    }

    pub(crate) fn pin_segment(&self, id: SegmentId) {
        if let Some(seg) = self.segment_ref(id) {
            seg.meta.pin_count.set(seg.meta.pin_count.get().saturating_add(1));
        }
    }

    pub(crate) fn unpin_segment(&self, id: SegmentId) {
        if let Some(seg) = self.segment_ref(id) {
            seg.meta.pin_count.set(seg.meta.pin_count.get().saturating_sub(1));
        }
    }
}

impl Drop for GcHeap {
    fn drop(&mut self) {
        if self.registered {
            registry::unregister(self.id);
        }
    }
}

/// 由 limb 指针读取头中的 [`HeapId`]（不校验堆是否仍存活）。
pub fn heap_id_for_limbs(limbs: NonNull<u64>) -> HeapId {
    // SAFETY: 调用方保证 ptr 指向本运行时分配的 limb 区。
    unsafe {
        let header = limbs.as_ptr().cast::<u8>().sub(AllocationHeader::size()).cast::<AllocationHeader>();
        (*header).heap_id
    }
}
