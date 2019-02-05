//! Scratch region：frame 生命周期，不参与普通 tracing。
#![allow(unsafe_code)]

use core::mem::MaybeUninit;

use crate::{
    budget::HeapBudget,
    error::{GcError, Result},
};

/// Scratch bump 水位标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScratchMark {
    cursor: usize,
}

/// 操作期内临时内存（`mark` / `rewind`）。
#[derive(Debug, Default)]
pub struct ScratchArena {
    buf: Vec<u8>,
    cursor: usize,
    peak_bytes: usize,
}

impl ScratchArena {
    /// 空 scratch。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前水位。
    pub fn mark(&self) -> ScratchMark {
        ScratchMark { cursor: self.cursor }
    }

    /// 回到标记（不缩小底层容量）。
    pub fn rewind(&mut self, mark: ScratchMark) {
        debug_assert!(mark.cursor <= self.cursor);
        self.cursor = mark.cursor.min(self.buf.len());
    }

    /// 已用字节。
    pub fn used_bytes(&self) -> usize {
        self.cursor
    }

    /// 峰值已用字节。
    pub fn peak_bytes(&self) -> usize {
        self.peak_bytes
    }

    /// 底层容量字节。
    pub fn capacity_bytes(&self) -> usize {
        self.buf.capacity()
    }

    /// 确保容量至少 `bytes`，并可选重置 cursor。
    pub fn ensure(&mut self, bytes: usize, budget: &HeapBudget, reset_cursor: bool) -> Result<()> {
        budget.check_scratch_bytes(0, bytes.max(1))?;
        if self.buf.capacity() < bytes {
            self.buf = Vec::with_capacity(bytes);
        }
        if self.buf.len() < bytes {
            self.buf.resize(bytes, 0);
        }
        if reset_cursor {
            self.cursor = 0;
        }
        Ok(())
    }

    /// Bump 分配未初始化字节切片。
    pub fn allocate_uninit(&mut self, bytes: usize, budget: &HeapBudget) -> Result<&mut [MaybeUninit<u8>]> {
        if bytes == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let end = self.cursor.checked_add(bytes).ok_or(GcError::InvalidCapacity)?;
        if end > self.buf.len() {
            // 尝试扩容到至少 end（仍受 budget 约束）。
            let need_cap = end.next_power_of_two().max(end);
            budget.check_scratch_bytes(0, need_cap)?;
            if self.buf.capacity() < need_cap {
                let mut grown = Vec::with_capacity(need_cap);
                grown.extend_from_slice(&self.buf[..self.cursor]);
                grown.resize(need_cap, 0);
                self.buf = grown;
            }
            else if self.buf.len() < need_cap {
                self.buf.resize(need_cap, 0);
            }
        }
        if end > self.buf.len() {
            return Err(GcError::ScratchUnderrun { need: bytes, remaining: self.buf.len().saturating_sub(self.cursor) });
        }
        let start = self.cursor;
        self.cursor = end;
        self.peak_bytes = self.peak_bytes.max(self.cursor);
        // SAFETY: MaybeUninit<u8> 与 u8 布局相同；调用方负责初始化。
        Ok(unsafe { core::slice::from_raw_parts_mut(self.buf.as_mut_ptr().add(start).cast::<MaybeUninit<u8>>(), bytes) })
    }

    /// Bump 分配并清零的 `u64` limb 切片。
    pub fn allocate_limbs_zeroed(&mut self, limbs: usize, budget: &HeapBudget) -> Result<&mut [u64]> {
        budget.check_limbs(limbs.max(1))?;
        let bytes = limbs.checked_mul(core::mem::size_of::<u64>()).ok_or(GcError::InvalidCapacity)?;
        // 对齐到 8。
        let align_pad = (8 - (self.cursor % 8)) % 8;
        if align_pad > 0 {
            let _ = self.allocate_uninit(align_pad, budget)?;
        }
        let slot = self.allocate_uninit(bytes, budget)?;
        for b in slot.iter_mut() {
            b.write(0);
        }
        // SAFETY: 已全部写 0；对齐满足 u64。
        Ok(unsafe { core::slice::from_raw_parts_mut(slot.as_mut_ptr().cast::<u64>(), limbs) })
    }

    /// 只读查看当前 scratch 中已初始化的一段（调用方保证范围在 cursor 内）。
    pub fn view_bytes(&self, start: usize, len: usize) -> Result<&[u8]> {
        let end = start.checked_add(len).ok_or(GcError::InvalidCapacity)?;
        if end > self.cursor {
            return Err(GcError::ScratchUnderrun { need: len, remaining: self.cursor.saturating_sub(start) });
        }
        Ok(&self.buf[start..end])
    }
}
