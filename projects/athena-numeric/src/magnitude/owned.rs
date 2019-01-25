//! System-heap `OwnedLimbBuffer`：allocation header + limb 区。
#![allow(unsafe_code)]

use core::{
    alloc::Layout,
    mem::{self, MaybeUninit},
    ptr::{self, NonNull},
};

use super::union::HeapPayload;

/// System allocator kind（写入 header；后续可扩展 arena）。
pub(crate) const ALLOC_KIND_SYSTEM: usize = 0;

/// `ptr` 前方的固定 header（不进入 `Magnitude` union）。
#[repr(C, align(8))]
struct AllocationHeader {
    kind: usize,
}

/// 拥有一段可写 limb 槽位（system heap）。
pub(crate) struct OwnedLimbBuffer {
    ptr: NonNull<u64>,
    capacity: usize,
}

impl OwnedLimbBuffer {
    /// 分配至少 `capacity` 个未初始化 `u64` 槽位（`capacity > 0`）。
    pub(crate) fn alloc_uninit(capacity: usize) -> Self {
        assert!(capacity > 0, "OwnedLimbBuffer capacity must be > 0");
        let layout = Self::block_layout(capacity);
        // SAFETY: layout 尺寸非零。
        let block = unsafe { std::alloc::alloc(layout) };
        if block.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        // SAFETY: 刚分配的对齐块可写 header。
        unsafe {
            ptr::write(block.cast::<AllocationHeader>(), AllocationHeader { kind: ALLOC_KIND_SYSTEM });
        }
        let limbs = unsafe { NonNull::new_unchecked(block.add(header_bytes()).cast::<u64>()) };
        Self { ptr: limbs, capacity }
    }

    /// 分配并拷贝 `src`（`src.len() <= capacity`）。
    pub(crate) fn alloc_copy(src: &[u64], capacity: usize) -> Self {
        assert!(capacity >= src.len());
        let buf = Self::alloc_uninit(capacity);
        // SAFETY: 新缓冲有 capacity 个槽位；src.len() <= capacity。
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), buf.ptr.as_ptr(), src.len());
        }
        buf
    }

    /// 转为 `HeapPayload`（所有权移出；调用方负责之后 `dealloc_heap`）。
    pub(crate) fn into_payload(self) -> HeapPayload {
        let payload = HeapPayload { ptr: self.ptr, capacity: self.capacity };
        mem::forget(self);
        payload
    }

    /// 从 `HeapPayload` 收回所有权。
    pub(crate) fn from_payload(payload: HeapPayload) -> Self {
        Self { ptr: payload.ptr, capacity: payload.capacity }
    }

    /// 释放 heap payload（system）。
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
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr().cast::<MaybeUninit<u64>>(), self.capacity) }
    }

    fn block_layout(capacity: usize) -> Layout {
        let header = Layout::new::<AllocationHeader>();
        let limbs = Layout::array::<u64>(capacity).expect("limb capacity layout");
        header.extend(limbs).expect("header+limbs layout").0.pad_to_align()
    }

    fn header_ptr(limbs: NonNull<u64>) -> *mut AllocationHeader {
        // SAFETY: limbs 由本模块分配，前方恰好一个 AllocationHeader。
        unsafe { limbs.as_ptr().cast::<u8>().sub(header_bytes()).cast::<AllocationHeader>() }
    }
}

#[inline]
fn header_bytes() -> usize {
    mem::size_of::<AllocationHeader>()
}

impl Drop for OwnedLimbBuffer {
    fn drop(&mut self) {
        let header = Self::header_ptr(self.ptr);
        // SAFETY: header 与本缓冲一同分配。
        let kind = unsafe { (*header).kind };
        debug_assert_eq!(kind, ALLOC_KIND_SYSTEM, "only system heap supported in this round");
        let layout = Self::block_layout(self.capacity);
        // SAFETY: 与 alloc 时同一 layout；block 起点为 header。
        unsafe {
            std::alloc::dealloc(header.cast::<u8>(), layout);
        }
    }
}
