//! 线程内 `GcHeap` 注册表（供 `Drop` 经 `heap_id` 回找 owner）。
//!
//! Session 合同为单线程；不用跨线程 `Mutex` + `Rc`。
//! TLS 析构顺序不定：所有入口用 `try_with`，线程退出时静默跳过（允许泄漏）。

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use crate::{
    error::{GcError, Result},
    heap::GcHeap,
    ids::HeapId,
};

struct RegistrySlot {
    heap: Weak<RefCell<GcHeap>>,
    /// 与 [`GcHeap`] 共享：`Drop` 遇 `HeapBusy` 时可无借 heap 地计数。
    drop_busy_leaks: Rc<Cell<u64>>,
}

#[derive(Default)]
struct Registry {
    slots: Vec<Option<RegistrySlot>>,
    free: Vec<u32>,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = const { RefCell::new(Registry { slots: Vec::new(), free: Vec::new() }) };
}

/// 登记共享 heap，返回稳定 [`HeapId`]。
pub fn register(heap: &Rc<RefCell<GcHeap>>, drop_busy_leaks: Rc<Cell<u64>>) -> HeapId {
    REGISTRY
        .try_with(|reg| {
            let mut reg = reg.borrow_mut();
            let index = if let Some(i) = reg.free.pop() {
                i
            }
            else {
                let i = u32::try_from(reg.slots.len()).expect("heap id overflow");
                reg.slots.push(None);
                i
            };
            reg.slots[index as usize] = Some(RegistrySlot { heap: Rc::downgrade(heap), drop_busy_leaks });
            HeapId(index)
        })
        .expect("register heap while TLS alive")
}

/// 注销（`GcHeap` 析构时调用）。
pub fn unregister(id: HeapId) {
    let _ = REGISTRY.try_with(|reg| {
        let mut reg = reg.borrow_mut();
        let index = id.0 as usize;
        if let Some(slot) = reg.slots.get_mut(index) {
            *slot = None;
            reg.free.push(id.0);
        }
    });
}

/// 升级弱引用并执行闭包。
pub fn with_heap<R>(id: HeapId, f: impl FnOnce(&mut GcHeap) -> R) -> Result<R> {
    let rc = REGISTRY
        .try_with(|reg| {
            let reg = reg.borrow();
            reg.slots
                .get(id.0 as usize)
                .and_then(|s| s.as_ref())
                .ok_or(GcError::UnknownAllocation)
                .and_then(|s| s.heap.upgrade().ok_or(GcError::UnknownAllocation))
        })
        .map_err(|_| GcError::RegistryUnavailable)??;
    let mut borrow = rc.try_borrow_mut().map_err(|_| GcError::HeapBusy)?;
    Ok(f(&mut borrow))
}

/// `Drop` 遇 `HeapBusy`：不借 `RefCell`，经共享计数器记一次泄漏。
pub fn record_drop_busy_leak(id: HeapId) {
    let _ = REGISTRY.try_with(|reg| {
        let reg = reg.borrow();
        if let Some(Some(slot)) = reg.slots.get(id.0 as usize) {
            slot.drop_busy_leaks.set(slot.drop_busy_leaks.get().saturating_add(1));
        }
    });
}
