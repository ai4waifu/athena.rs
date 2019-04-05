//! 段槽位 / bump 推进 / 回收（不知根或数学对象）。

use core::cell::Cell;

use crate::{
    error::Result,
    ids::SegmentId,
    segment::{SegmentKind, SegmentMeta},
};

use super::{allocation::align_up, state::GcHeap};

/// 默认段容量。
pub(super) const DEFAULT_SEGMENT_BYTES: usize = 256 * 1024;

pub(super) struct SegmentStorage {
    pub(super) meta: SegmentMeta,
    pub(super) bytes: Vec<u8>,
}

impl GcHeap {
    /// 存活段数量。
    pub fn segments(&self) -> impl Iterator<Item = &SegmentMeta> {
        self.segments.iter().filter_map(|s| s.as_ref().map(|x| &x.meta))
    }

    /// 驻留字节数。
    pub fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    pub(super) fn bump_allocate(&mut self, kind: SegmentKind, bytes: usize) -> Result<(usize, usize)> {
        for (index, slot) in self.segments.iter_mut().enumerate() {
            let Some(seg) = slot.as_mut()
            else {
                continue;
            };
            if seg.meta.kind != kind {
                continue;
            }
            let aligned_used = align_up(seg.meta.used, 8);
            if aligned_used.saturating_add(bytes) <= seg.meta.capacity {
                seg.meta.used = aligned_used + bytes;
                return Ok((index, aligned_used));
            }
        }
        let capacity = bytes.max(DEFAULT_SEGMENT_BYTES).next_power_of_two();
        let index = self.alloc_segment(kind, capacity)?;
        let seg = self.segments[index].as_mut().expect("new segment");
        seg.meta.used = bytes;
        Ok((index, 0))
    }

    pub(super) fn alloc_segment(&mut self, kind: SegmentKind, capacity: usize) -> Result<usize> {
        let count = self.segments.iter().filter(|s| s.is_some()).count() + 1;
        self.budget.check_segment_count(count)?;
        self.budget.check_arena_bytes(self.resident_bytes, capacity)?;
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let index = if let Some(free) = self.free_segment_slots.pop() {
            free
        }
        else {
            let i = self.segments.len();
            self.segments.push(None);
            i
        };
        let id = SegmentId { index: index as u32, generation };
        self.segments[index] = Some(SegmentStorage {
            meta: SegmentMeta {
                id,
                kind,
                capacity,
                used: 0,
                live_count: 0,
                pin_count: Cell::new(0),
                last_access: self.access_clock,
            },
            bytes: vec![0u8; capacity],
        });
        self.resident_bytes = self.resident_bytes.saturating_add(capacity);
        self.stats.peak_arena_bytes = self.stats.peak_arena_bytes.max(self.resident_bytes);
        self.stats.segments_allocated = self.stats.segments_allocated.saturating_add(1);
        Ok(index)
    }

    pub(super) fn try_reclaim_segment(&mut self, id: SegmentId) -> bool {
        let Some(index) = self.resolve_index(id)
        else {
            return false;
        };
        let Some(seg) = self.segments[index].as_ref()
        else {
            return false;
        };
        if !seg.meta.is_reclaimable() {
            return false;
        }
        let capacity = seg.meta.capacity;
        self.segments[index] = None;
        self.free_segment_slots.push(index);
        self.resident_bytes = self.resident_bytes.saturating_sub(capacity);
        true
    }

    pub(super) fn resolve_index(&self, id: SegmentId) -> Option<usize> {
        let index = id.index as usize;
        let seg = self.segments.get(index)?.as_ref()?;
        (seg.meta.id == id).then_some(index)
    }

    pub(super) fn segment_ref(&self, id: SegmentId) -> Option<&SegmentStorage> {
        let index = self.resolve_index(id)?;
        self.segments[index].as_ref()
    }

    pub(super) fn segment_mut(&mut self, id: SegmentId) -> Option<&mut SegmentStorage> {
        let index = self.resolve_index(id)?;
        self.segments[index].as_mut()
    }

    pub(super) fn touch(&mut self, index: usize) {
        self.access_clock = self.access_clock.wrapping_add(1);
        if let Some(seg) = self.segments[index].as_mut() {
            seg.meta.last_access = self.access_clock;
        }
    }
}
