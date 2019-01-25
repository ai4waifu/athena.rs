//! 纯 `union Magnitude` 与 `HeapPayload`（无 tag）。
#![allow(unsafe_code)]

use core::ptr::NonNull;

/// 堆载荷：仅 `ptr + capacity`；`len` 在外层 `meta`，owner 在 allocation header。
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct HeapPayload {
    /// 指向 limb 数组首元素（header 之后）。
    pub(crate) ptr: NonNull<u64>,
    /// 可写 limb 槽位数。
    pub(crate) capacity: usize,
}

/// 持久 magnitude 的纯 storage union（16 bytes on LP64）。
///
/// 禁止在未核对 `meta.mode` 的情况下读取字段。
#[repr(C)]
pub(crate) union Magnitude {
    pub(crate) limb1: u64,
    pub(crate) limb2: [u64; 2],
    pub(crate) heap: HeapPayload,
}

// `Magnitude` 仅含 Copy 字段；手动实现以便在 Tagged 路径外不轻易复制未配对状态。
impl Clone for Magnitude {
    #[inline]
    fn clone(&self) -> Self {
        // 按位复制整个 union 槽位；语义正确性由外层 meta 保证。
        *self
    }
}

impl Copy for Magnitude {}
