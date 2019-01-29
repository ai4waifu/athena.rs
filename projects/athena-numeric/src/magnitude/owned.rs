//! `OwnedLimbBuffer`：经 `athena-gc` 分配的 limb 区（header 在 `athena-gc`）。
#![allow(unsafe_code)]

use core::{
    mem::{self, MaybeUninit},
    ptr::NonNull,
};
use std::cell::RefCell;
use std::rc::Rc;

use athena_gc::{GcError, GcHeap, HeapId, heap_id_for_limbs};

use super::union::HeapPayload;

/// 拥有一段可写 limb 槽位（`GcHeap` numeric segment）。
pub(crate) struct OwnedLimbBuffer {
    ptr: NonNull<u64>,
    capacity: usize,
    heap_id: HeapId,
}

impl OwnedLimbBuffer {
    /// 在线程默认 heap 上分配。
    pub(crate) fn alloc_uninit(capacity: usize) -> Self {
        Self::alloc_uninit_in(&GcHeap::shared_default(), capacity)
            .unwrap_or_else(|e| panic!("gc numeric alloc failed: {e}"))
    }

    /// 在指定 heap 上分配。
    pub(crate) fn alloc_uninit_in(heap: &Rc<RefCell<GcHeap>>, capacity: usize) -> athena_gc::Result<Self> {
        assert!(capacity > 0, "OwnedLimbBuffer capacity must be > 0");
        let block = heap.borrow_mut().allocate_numeric_block(capacity)?;
        Ok(Self {
            ptr: block.ptr,
            capacity: block.capacity,
            heap_id: block.heap_id,
        })
    }

    /// 经 `HeapId` 分配（Clone 同堆）。
    pub(crate) fn alloc_uninit_on(heap_id: HeapId, capacity: usize) -> athena_gc::Result<Self> {
        assert!(capacity > 0, "OwnedLimbBuffer capacity must be > 0");
        athena_gc::with_registered_heap(heap_id, |heap| {
            let block = heap.allocate_numeric_block(capacity)?;
            Ok(Self {
                ptr: block.ptr,
                capacity: block.capacity,
                heap_id: block.heap_id,
            })
        })
    }

    /// 分配并拷贝 `src`（默认 heap）。
    pub(crate) fn alloc_copy(src: &[u64], capacity: usize) -> Self {
        Self::alloc_copy_in(&GcHeap::shared_default(), src, capacity)
            .unwrap_or_else(|e| panic!("gc numeric alloc failed: {e}"))
    }

    /// 在指定 heap 上分配并拷贝。
    pub(crate) fn alloc_copy_in(
        heap: &Rc<RefCell<GcHeap>>,
        src: &[u64],
        capacity: usize,
    ) -> athena_gc::Result<Self> {
        assert!(capacity >= src.len());
        let buf = Self::alloc_uninit_in(heap, capacity)?;
        // SAFETY: 新缓冲有 capacity 个槽位；src.len() <= capacity。
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), buf.ptr.as_ptr(), src.len());
        }
        Ok(buf)
    }

    /// 同堆拷贝。
    pub(crate) fn alloc_copy_on(heap_id: HeapId, src: &[u64], capacity: usize) -> athena_gc::Result<Self> {
        assert!(capacity >= src.len());
        let buf = Self::alloc_uninit_on(heap_id, capacity)?;
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), buf.ptr.as_ptr(), src.len());
        }
        Ok(buf)
    }

    /// 转为 `HeapPayload`（所有权移出；调用方负责之后 `dealloc_heap`）。
    pub(crate) fn into_payload(self) -> HeapPayload {
        let payload = HeapPayload {
            ptr: self.ptr,
            capacity: self.capacity,
        };
        mem::forget(self);
        payload
    }

    /// 从 `HeapPayload` 收回所有权（从 header 读 `HeapId`）。
    pub(crate) fn from_payload(payload: HeapPayload) -> Self {
        let heap_id = heap_id_for_limbs(payload.ptr);
        Self {
            ptr: payload.ptr,
            capacity: payload.capacity,
            heap_id,
        }
    }

    /// 释放 heap payload。
    pub(crate) fn dealloc_heap(payload: HeapPayload) {
        let buf = Self::from_payload(payload);
        drop(buf);
    }

    /// 只读视图（调用方保证 `len <= capacity` 且已初始化）。
    pub(crate) fn as_slice(&self, len: usize) -> &[u64] {
        debug_assert!(len <= self.capacity);
        // SAFETY: 调用方保证前 len 个 limb 已初始化。
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), len) }
    }

    /// 可写视图。
    pub(crate) fn as_mut_slice(&mut self, len: usize) -> &mut [u64] {
        debug_assert!(len <= self.capacity);
        // SAFETY: 调用方保证前 len 个槽位可写。
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), len) }
    }

    /// 未初始化可写区（整段 capacity）。
    pub(crate) fn as_mut_uninit(&mut self) -> &mut [MaybeUninit<u64>] {
        // SAFETY: MaybeUninit 与 u64 布局相同；调用方负责初始化。
        unsafe {
            core::slice::from_raw_parts_mut(self.ptr.as_ptr().cast::<MaybeUninit<u64>>(), self.capacity)
        }
    }

    /// Owner heap id。
    pub(crate) fn heap_id(&self) -> HeapId {
        self.heap_id
    }
}

impl Drop for OwnedLimbBuffer {
    fn drop(&mut self) {
        match GcHeap::release_numeric_limbs_registered(self.heap_id, self.ptr) {
            Ok(()) | Err(GcError::RegistryUnavailable) | Err(GcError::UnknownAllocation) => {}
            Err(GcError::HeapBusy) => {
                // 重入 Drop：保留泄漏，避免 panic。
            }
            Err(_) => {}
        }
    }
}
