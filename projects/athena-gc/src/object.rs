//! Object arena：带 generation 的 `GcObjectId` 槽位。

use core::cell::Cell;
use core::ptr::NonNull;

use crate::error::{GcError, Result};
use crate::header::MarkState;
use crate::ids::{GcObjectId, SegmentId};

/// Object segment 中的一块 payload。
#[derive(Debug, Clone, Copy)]
pub struct ObjectBlock {
    /// Payload 起点（header 之前缀）。
    pub ptr: NonNull<u8>,
    /// Payload 字节数。
    pub byte_len: usize,
    /// 所属 segment。
    pub segment_id: SegmentId,
}

#[derive(Debug)]
pub(crate) struct ObjectSlot {
    pub generation: u32,
    pub mark: Cell<MarkState>,
    pub block: Option<ObjectBlock>,
}

impl ObjectSlot {
    pub(crate) fn vacant(generation: u32) -> Self {
        Self {
            generation,
            mark: Cell::new(MarkState::White),
            block: None,
        }
    }
}

/// 解析结果。
pub(crate) fn resolve_slot(
    slots: &[Option<ObjectSlot>],
    id: GcObjectId,
) -> Result<&ObjectSlot> {
    let slot = slots
        .get(id.index as usize)
        .and_then(|s| s.as_ref())
        .ok_or(GcError::StaleObject {
            index: id.index,
            expected_generation: id.generation,
        })?;
    if slot.generation != id.generation || slot.block.is_none() {
        return Err(GcError::StaleObject {
            index: id.index,
            expected_generation: id.generation,
        });
    }
    Ok(slot)
}
