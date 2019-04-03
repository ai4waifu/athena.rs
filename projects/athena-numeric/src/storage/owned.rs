//! `OwnedLimbBuffer`：经 `athena-gc` 分配的 limb 区（header 在 `athena-gc`）。
#![allow(unsafe_code)]

use core::{
    mem::{self, MaybeUninit},
    ptr::NonNull,
};
use std::{cell::RefCell, rc::Rc};

use athena_gc::{GcError, GcHeap, HeapId, heap_id_for_limbs};

use super::union::HeapPayload;

/// 拥有一段可写 limb 槽位（`GcHeap` numeric segment）。
pub(crate) struct OwnedLimbBuffer {
    ptr: NonNull<u64>,
    capacity: usize,
    heap_id: HeapId,
}

impl OwnedLimbBuffer {
    /// 在指定 heap 上分配（[`NumericOwnership::RustOwned`]）。
    pub(crate) fn alloc_uninit_in(heap: &Rc<RefCell<GcHeap>>, capacity: usize) -> athena_gc::Result<Self> {
        Self::alloc_uninit_in_with(heap, capacity, false)
    }

    /// 在指定 heap 上分配 GC-owned block（Session / 长期值发布）。
    pub(crate) fn alloc_uninit_gc_owned_in(heap: &Rc<RefCell<GcHeap>>, capacity: usize) -> athena_gc::Result<Self> {
        Self::alloc_uninit_in_with(heap, capacity, true)
    }

    /// GC-owned 分配：`allocate_traced_numeric` + 登记一条 [`athena_gc::NumericRoot`]。
    fn alloc_uninit_in_with(heap: &Rc<RefCell<GcHeap>>, capacity: usize, gc_owned: bool) -> athena_gc::Result<Self> {
        if capacity == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let mut h = heap.borrow_mut();
        let block = if gc_owned {
            let block = h.allocate_traced_numeric(capacity)?;
            let _ = h.register_numeric_root(block.ptr, athena_gc::RootKind::Numeric)?;
            block
        }
        else {
            h.allocate_numeric_block(capacity)?
        };
        Ok(Self { ptr: block.ptr, capacity: block.capacity, heap_id: block.heap_id })
    }

    /// 批内分配：已持有 `&mut GcHeap`（无 `RefCell`）。
    pub(crate) fn alloc_uninit_mut(heap: &mut GcHeap, capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let block = heap.allocate_numeric_block(capacity)?;
        Ok(Self { ptr: block.ptr, capacity: block.capacity, heap_id: block.heap_id })
    }

    /// 经 `HeapId` 分配（Clone 同堆 · RustOwned）。
    pub(crate) fn alloc_uninit_on(heap_id: HeapId, capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 {
            return Err(GcError::InvalidCapacity);
        }
        athena_gc::with_registered_heap(heap_id, |heap| {
            let block = heap.allocate_numeric_block(capacity)?;
            Ok(Self { ptr: block.ptr, capacity: block.capacity, heap_id: block.heap_id })
        })
    }

    /// 在指定 heap 上分配并拷贝（RustOwned）。
    pub(crate) fn alloc_copy_in(heap: &Rc<RefCell<GcHeap>>, src: &[u64], capacity: usize) -> athena_gc::Result<Self> {
        Self::alloc_copy_in_with(heap, src, capacity, false)
    }

    /// GC-owned 分配并拷贝。
    pub(crate) fn alloc_copy_gc_owned_in(heap: &Rc<RefCell<GcHeap>>, src: &[u64], capacity: usize) -> athena_gc::Result<Self> {
        Self::alloc_copy_in_with(heap, src, capacity, true)
    }

    fn alloc_copy_in_with(heap: &Rc<RefCell<GcHeap>>, src: &[u64], capacity: usize, gc_owned: bool) -> athena_gc::Result<Self> {
        if capacity == 0 || capacity < src.len() {
            return Err(GcError::InvalidCapacity);
        }
        let buf = Self::alloc_uninit_in_with(heap, capacity, gc_owned)?;
        // SAFETY: 新缓冲有 capacity 个槽位；src.len() <= capacity。
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), buf.ptr.as_ptr(), src.len());
        }
        Ok(buf)
    }

    /// 同堆拷贝。
    pub(crate) fn alloc_copy_on(heap_id: HeapId, src: &[u64], capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 || capacity < src.len() {
            return Err(GcError::InvalidCapacity);
        }
        let buf = Self::alloc_uninit_on(heap_id, capacity)?;
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), buf.ptr.as_ptr(), src.len());
        }
        Ok(buf)
    }

    /// 转为 `HeapPayload`（所有权移出；调用方负责之后 `dealloc_heap`）。
    pub(crate) fn into_payload(self) -> HeapPayload {
        let payload = HeapPayload { ptr: self.ptr, capacity: self.capacity };
        mem::forget(self);
        payload
    }

    /// 从 `HeapPayload` 收回所有权（从 header 读 `HeapId`）。
    pub(crate) fn from_payload(payload: HeapPayload) -> Self {
        let heap_id = heap_id_for_limbs(payload.ptr);
        Self { ptr: payload.ptr, capacity: payload.capacity, heap_id }
    }

    /// 释放 heap payload。
    pub(crate) fn dealloc_heap(payload: HeapPayload) {
        let buf = Self::from_payload(payload);
        drop(buf);
    }

    /// 只读视图（`len` 截断到 `capacity`，禁止构造越界 slice）。
    pub(crate) fn as_slice(&self, len: usize) -> &[u64] {
        let n = len.min(self.capacity);
        // SAFETY: n <= capacity；前 n 个 limb 由调用方保证已初始化（或为零填充）。
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), n) }
    }

    /// 可写视图（`len` 截断到 `capacity`）。
    pub(crate) fn as_mut_slice(&mut self, len: usize) -> &mut [u64] {
        let n = len.min(self.capacity);
        // SAFETY: n <= capacity。
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), n) }
    }

    /// 未初始化可写区（整段 capacity）。
    pub(crate) fn as_mut_uninit(&mut self) -> &mut [MaybeUninit<u64>] {
        // SAFETY: MaybeUninit 与 u64 布局相同；调用方负责初始化。
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr().cast::<MaybeUninit<u64>>(), self.capacity) }
    }

    /// Owner heap id。
    pub(crate) fn heap_id(&self) -> HeapId {
        self.heap_id
    }

    /// 分配容量（limb 数）。
    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }
}

impl Drop for OwnedLimbBuffer {
    fn drop(&mut self) {
        match GcHeap::release_numeric_limbs_registered(self.heap_id, self.ptr) {
            Ok(()) | Err(GcError::RegistryUnavailable) | Err(GcError::UnknownAllocation) => {}
            Err(GcError::HeapBusy) => {
                athena_gc::record_drop_busy_leak(self.heap_id);
            }
            Err(GcError::LifecycleMismatch) => {
                // Unspecified / 混用：release 已计入 stats，不得再 free。
            }
            Err(_) => {}
        }
    }
}
