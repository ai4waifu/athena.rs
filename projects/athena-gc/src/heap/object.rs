//! 对象区：分配、读写载荷、显式释放。

use core::cell::Cell;

use crate::{
    error::{GcError, Result},
    header::{BlockKind, MarkState, NumericOwnership},
    ids::GcObjectId,
    object::{ObjectBlock, ObjectSlot, resolve_slot},
    segment::SegmentKind,
};

use super::state::GcHeap;

impl GcHeap {
    /// 分配对象槽与载荷，返回 [`GcObjectId`]。
    pub fn allocate_object(&mut self, payload_bytes: usize) -> Result<GcObjectId> {
        if payload_bytes == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let index = if let Some(i) = self.free_objects.pop() {
            i
        }
        else {
            let i = u32::try_from(self.objects.len()).map_err(|_| GcError::InvalidCapacity)?;
            self.objects.push(None);
            i
        };
        let generation = self.objects.get(index as usize).and_then(|s| s.as_ref().map(|o| o.generation.wrapping_add(1).max(1))).unwrap_or(1);
        let (seg_id, ptr) =
            self.allocate_payload(SegmentKind::LongLivedObject, BlockKind::Object, payload_bytes, index, NumericOwnership::Unspecified)?;
        self.objects[index as usize] = Some(ObjectSlot {
            generation,
            mark: Cell::new(MarkState::White),
            block: Some(ObjectBlock { ptr, byte_len: payload_bytes, segment_id: seg_id }),
        });
        Ok(GcObjectId { index, generation })
    }

    /// 解析对象载荷（可变）。
    pub fn object_payload_mut(&mut self, id: GcObjectId) -> Result<&mut [u8]> {
        let block = {
            let slot = resolve_slot(&self.objects, id)?;
            slot.block.ok_or(GcError::StaleObject { index: id.index, expected_generation: id.generation })?
        };
        // SAFETY: block 由本 heap 分配且 slot 仍存活。
        Ok(unsafe { core::slice::from_raw_parts_mut(block.ptr.as_ptr(), block.byte_len) })
    }

    /// 解析对象载荷（只读）。
    pub fn object_payload(&self, id: GcObjectId) -> Result<&[u8]> {
        let block = {
            let slot = resolve_slot(&self.objects, id)?;
            slot.block.ok_or(GcError::StaleObject { index: id.index, expected_generation: id.generation })?
        };
        Ok(unsafe { core::slice::from_raw_parts(block.ptr.as_ptr(), block.byte_len) })
    }

    /// 显式释放对象（推进 generation，可供陈旧检测）。
    pub fn release_object(&mut self, id: GcObjectId) -> Result<()> {
        let index = id.index as usize;
        let Some(Some(slot)) = self.objects.get(index)
        else {
            return Err(GcError::StaleObject { index: id.index, expected_generation: id.generation });
        };
        if slot.generation != id.generation || slot.block.is_none() {
            return Err(GcError::StaleObject { index: id.index, expected_generation: id.generation });
        }
        if let Some(mut slot) = self.objects[index].take() {
            if let Some(block) = slot.block.take() {
                let _ = self.release_payload(block.ptr, BlockKind::Object);
            }
            let generation = slot.generation.wrapping_add(1).max(1);
            self.objects[index] = Some(ObjectSlot::vacant(generation));
            self.free_objects.push(id.index);
        }
        Ok(())
    }
}
