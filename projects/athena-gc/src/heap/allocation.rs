//! 分配头 / 载荷 / 标记原语（堆内主要 unsafe 边界）。
#![allow(unsafe_code)]

use core::ptr::NonNull;

use crate::{
    batch::AllocationAccounting,
    error::{GcError, Result},
    header::{AllocationHeader, BlockKind, MarkState, ReclaimAuthority},
    ids::SegmentId,
    mode::GcMode,
    segment::SegmentKind,
};

use super::state::GcHeap;

/// `AllocationHeader::pin_state` 哨兵：块已释放，禁止再解析为存活分配。
pub(super) const FREED_PIN_SENTINEL: u16 = u16::MAX;

pub(super) fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

impl GcHeap {
    /// 分配头（limb 或对象载荷起点）。
    pub fn header_for_payload(&self, payload: NonNull<u8>) -> Result<&AllocationHeader> {
        let header = unsafe { payload.as_ptr().sub(AllocationHeader::size()).cast::<AllocationHeader>() };
        let hdr = unsafe { &*header };
        if hdr.heap_id != self.id {
            return Err(GcError::UnknownAllocation);
        }
        if hdr.pin_state == FREED_PIN_SENTINEL {
            return Err(GcError::UnknownAllocation);
        }
        self.segment_ref(hdr.segment_id).ok_or(GcError::UnknownAllocation)?;
        Ok(hdr)
    }

    /// 兼容旧名。
    pub fn header_for_limbs(&self, limbs: NonNull<u64>) -> Result<&AllocationHeader> {
        self.header_for_payload(limbs.cast())
    }

    /// 可变分配头（供后续数值 / 所有权路径使用）。
    #[allow(dead_code)]
    pub(super) fn header_mut_for_limbs(&mut self, limbs: NonNull<u64>) -> Result<&mut AllocationHeader> {
        let header = unsafe { limbs.as_ptr().sub(AllocationHeader::size()).cast::<AllocationHeader>() };
        let hdr = unsafe { &mut *header };
        if hdr.pin_state == FREED_PIN_SENTINEL {
            return Err(GcError::UnknownAllocation);
        }
        self.segment_ref(hdr.segment_id).ok_or(GcError::UnknownAllocation)?;
        Ok(hdr)
    }

    /// 标记分配可达。
    pub fn mark_payload(&mut self, payload: NonNull<u8>) -> Result<()> {
        let header = unsafe { payload.as_ptr().sub(AllocationHeader::size()).cast::<AllocationHeader>() };
        unsafe {
            (*header).mark_state = MarkState::Black;
            if (*header).block_kind == BlockKind::Object {
                let idx = (*header).object_index as usize;
                if let Some(Some(slot)) = self.objects.get(idx) {
                    slot.mark.set(MarkState::Black);
                }
            }
        }
        Ok(())
    }

    /// 标记 limbs 可达。
    pub fn mark_limbs(&mut self, limbs: NonNull<u64>) -> Result<()> {
        self.mark_payload(limbs.cast())
    }

    pub(super) fn allocate_payload(
        &mut self,
        kind: SegmentKind,
        block_kind: BlockKind,
        payload_bytes: usize,
        object_index: u32,
        reclaim_authority: ReclaimAuthority,
    ) -> Result<(SegmentId, NonNull<u8>)> {
        let total = AllocationHeader::size().checked_add(payload_bytes).ok_or(GcError::InvalidCapacity)?;
        let (seg_index, offset) = self.bump_allocate(kind, total)?;
        let seg = self.segments[seg_index].as_mut().expect("segment");
        let seg_id = seg.meta.id;
        let header_ptr = unsafe { seg.bytes.as_mut_ptr().add(offset).cast::<AllocationHeader>() };
        unsafe {
            header_ptr.write(AllocationHeader {
                segment_id: seg_id,
                heap_id: self.id,
                block_kind,
                byte_len: u32::try_from(payload_bytes).map_err(|_| GcError::InvalidCapacity)?,
                alignment: 8,
                mark_state: MarkState::White,
                pin_state: 0,
                object_index,
                reclaim_authority,
            });
        }
        let payload = unsafe { NonNull::new_unchecked(seg.bytes.as_mut_ptr().add(offset + AllocationHeader::size())) };
        seg.meta.live_count = seg.meta.live_count.saturating_add(1);
        match self.accounting {
            AllocationAccounting::Full => {
                self.touch(seg_index);
                self.controller.record_allocation(total);
                self.stats.allocation_count = self.stats.allocation_count.saturating_add(1);
                self.stats.total_arena_bytes_allocated = self.stats.total_arena_bytes_allocated.saturating_add(total);
                if self.controller.should_collect_after_alloc() {
                    let _ = self.collect();
                }
            }
            AllocationAccounting::Batched => {
                self.batch_bytes = self.batch_bytes.saturating_add(total);
                self.batch_allocs = self.batch_allocs.saturating_add(1);
            }
            AllocationAccounting::Off => {}
        }
        Ok((seg_id, payload))
    }

    pub(super) fn release_payload(&mut self, payload: NonNull<u8>, expected: BlockKind) -> Result<()> {
        let header = unsafe { payload.as_ptr().sub(AllocationHeader::size()).cast::<AllocationHeader>() };
        let (seg_id, kind, pin) = unsafe { ((*header).segment_id, (*header).block_kind, (*header).pin_state) };
        if kind != expected || pin == FREED_PIN_SENTINEL {
            return Err(GcError::UnknownAllocation);
        }
        let Some(seg) = self.segment_mut(seg_id)
        else {
            return Err(GcError::UnknownAllocation);
        };
        if seg.meta.live_count > 0 {
            seg.meta.live_count -= 1;
        }
        unsafe {
            (*header).mark_state = MarkState::White;
            // 保留 byte_len 供 segment 遍历；用 pin 哨兵标记已释放。
            (*header).pin_state = FREED_PIN_SENTINEL;
        }
        if matches!(self.controller.effective_mode(), GcMode::Auto) {
            self.try_reclaim_segment(seg_id);
        }
        Ok(())
    }
}
