//! 临时 / 已发布 limb 缓冲句柄（Living `24`：Drop 不猜 reclaim 类别）。
#![allow(unsafe_code)]

use core::{
    mem::{self, MaybeUninit},
    ptr::NonNull,
};
use std::{cell::RefCell, rc::Rc};

use athena_gc::{GcError, GcHeap, HeapId, heap_id_for_limbs};

use super::union::HeapPayload;

/// 临时 ExplicitRelease 缓冲。`Drop` 只走显式 release，不查/猜 tracing 路径。
pub(crate) struct OwnedLimbBuffer {
    ptr: NonNull<u64>,
    capacity: usize,
    heap_id: HeapId,
}

/// 已发布 TracingSweep 缓冲（持有一条 [`athena_gc::NumericRoot`]）。
///
/// `Drop` 只撤 root，不 free block。
pub(crate) struct RootedLimbBuffer {
    ptr: NonNull<u64>,
    capacity: usize,
    heap_id: HeapId,
}

macro_rules! limb_buffer_views {
    ($name:ident) => {
        impl $name {
            /// 只读视图（`len` 截断到 `capacity`）。
            pub(crate) fn as_slice(&self, len: usize) -> &[u64] {
                let n = len.min(self.capacity);
                unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), n) }
            }

            /// 可写视图（`len` 截断到 `capacity`）。
            pub(crate) fn as_mut_slice(&mut self, len: usize) -> &mut [u64] {
                let n = len.min(self.capacity);
                unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), n) }
            }

            /// 未初始化可写区（整段 capacity）。
            pub(crate) fn as_mut_uninit(&mut self) -> &mut [MaybeUninit<u64>] {
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

            /// 转为 `HeapPayload`（所有权移出）。
            pub(crate) fn into_payload(self) -> HeapPayload {
                let payload = HeapPayload { ptr: self.ptr, capacity: self.capacity };
                mem::forget(self);
                payload
            }

            /// 从 `HeapPayload` 收回（调用方必须保证 reclaim 类别与本类型一致）。
            pub(crate) fn from_payload(payload: HeapPayload) -> Self {
                let heap_id = heap_id_for_limbs(payload.ptr);
                Self { ptr: payload.ptr, capacity: payload.capacity, heap_id }
            }
        }
    };
}

limb_buffer_views!(OwnedLimbBuffer);
limb_buffer_views!(RootedLimbBuffer);

impl OwnedLimbBuffer {
    /// 在指定 heap 上分配临时块。
    pub(crate) fn alloc_uninit_in(heap: &Rc<RefCell<GcHeap>>, capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let mut h = heap.borrow_mut();
        let block = h.allocate_numeric_block(capacity)?;
        Ok(Self { ptr: block.ptr, capacity: block.capacity, heap_id: block.heap_id })
    }

    /// 批内分配临时块。
    pub(crate) fn alloc_uninit_mut(heap: &mut GcHeap, capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let block = heap.allocate_numeric_block(capacity)?;
        Ok(Self { ptr: block.ptr, capacity: block.capacity, heap_id: block.heap_id })
    }

    /// 经 `HeapId` 分配临时块。
    pub(crate) fn alloc_uninit_on(heap_id: HeapId, capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 {
            return Err(GcError::InvalidCapacity);
        }
        athena_gc::with_registered_heap(heap_id, |heap| {
            let block = heap.allocate_numeric_block(capacity)?;
            Ok(Self { ptr: block.ptr, capacity: block.capacity, heap_id: block.heap_id })
        })
    }

    /// 分配并拷贝（临时）。
    pub(crate) fn alloc_copy_in(heap: &Rc<RefCell<GcHeap>>, src: &[u64], capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 || capacity < src.len() {
            return Err(GcError::InvalidCapacity);
        }
        let mut buf = Self::alloc_uninit_in(heap, capacity)?;
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), buf.ptr.as_ptr(), src.len());
        }
        Ok(buf)
    }

    /// 同堆拷贝（临时；`try_clone_in` 深复制路径）。
    pub(crate) fn alloc_copy_on(heap_id: HeapId, src: &[u64], capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 || capacity < src.len() {
            return Err(GcError::InvalidCapacity);
        }
        let mut buf = Self::alloc_uninit_on(heap_id, capacity)?;
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), buf.ptr.as_ptr(), src.len());
        }
        Ok(buf)
    }

    /// 释放临时 heap payload。
    pub(crate) fn dealloc_heap(payload: HeapPayload) {
        drop(Self::from_payload(payload));
    }
}

impl RootedLimbBuffer {
    /// 分配已发布块并登记一条 [`athena_gc::NumericRoot`]。
    pub(crate) fn alloc_uninit_in(heap: &Rc<RefCell<GcHeap>>, capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let mut h = heap.borrow_mut();
        let block = h.allocate_traced_numeric(capacity)?;
        let _ = h.register_numeric_root(&block, athena_gc::RootKind::Numeric)?;
        Ok(Self { ptr: block.ptr, capacity: block.capacity, heap_id: block.heap_id })
    }

    /// 经 `HeapId` 分配已发布块并登记 root。
    pub(crate) fn alloc_uninit_on(heap_id: HeapId, capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 {
            return Err(GcError::InvalidCapacity);
        }
        athena_gc::with_registered_heap(heap_id, |heap| {
            let block = heap.allocate_traced_numeric(capacity)?;
            let _ = heap.register_numeric_root(&block, athena_gc::RootKind::Numeric)?;
            Ok(Self { ptr: block.ptr, capacity: block.capacity, heap_id: block.heap_id })
        })
    }

    /// 分配并拷贝（已发布 + root）。
    pub(crate) fn alloc_copy_in(heap: &Rc<RefCell<GcHeap>>, src: &[u64], capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 || capacity < src.len() {
            return Err(GcError::InvalidCapacity);
        }
        let mut buf = Self::alloc_uninit_in(heap, capacity)?;
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), buf.ptr.as_ptr(), src.len());
        }
        Ok(buf)
    }

    /// 同堆或经 `HeapId` 深复制为已发布块（Living `31`：`try_clone_in` 路径）。
    pub(crate) fn alloc_copy_on(heap_id: HeapId, src: &[u64], capacity: usize) -> athena_gc::Result<Self> {
        if capacity == 0 || capacity < src.len() {
            return Err(GcError::InvalidCapacity);
        }
        let mut buf = Self::alloc_uninit_on(heap_id, capacity)?;
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), buf.ptr.as_ptr(), src.len());
        }
        Ok(buf)
    }

    /// 结束已发布 payload 的 root 责任（不 free）。
    pub(crate) fn dealloc_heap(payload: HeapPayload) {
        drop(Self::from_payload(payload));
    }
}

impl Drop for OwnedLimbBuffer {
    fn drop(&mut self) {
        match GcHeap::release_temporary_numeric_registered(self.heap_id, self.ptr) {
            Ok(()) | Err(GcError::RegistryUnavailable) | Err(GcError::UnknownAllocation) => {}
            Err(GcError::HeapBusy) => {
                athena_gc::record_drop_busy_leak(self.heap_id);
            }
            Err(GcError::LifecycleMismatch) => {}
            Err(_) => {}
        }
    }
}

impl Drop for RootedLimbBuffer {
    fn drop(&mut self) {
        match GcHeap::unregister_one_numeric_root_registered(self.heap_id, self.ptr) {
            Ok(()) | Err(GcError::RegistryUnavailable) | Err(GcError::UnknownAllocation) => {}
            Err(GcError::HeapBusy) => {
                athena_gc::record_drop_busy_leak(self.heap_id);
            }
            Err(GcError::LifecycleMismatch) => {}
            Err(_) => {}
        }
    }
}
