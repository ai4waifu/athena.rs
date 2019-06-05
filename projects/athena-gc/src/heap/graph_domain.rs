//! 图索引 / 属性域载荷（兼容包装，中期可迁 `athena-graph`）。

use core::ptr::NonNull;

use crate::{
    error::{GcError, Result},
    header::{BlockKind, NumericOwnership},
    ids::{HeapId, SegmentId},
    segment::SegmentKind,
};

use super::state::GcHeap;

/// 图索引 / 属性域载荷（类型化段，非独立分配器）。
#[derive(Debug, Clone, Copy)]
pub struct GraphDomainBlock {
    /// 载荷起点。
    pub ptr: NonNull<u8>,
    /// 字节长度。
    pub byte_len: usize,
    /// 所属段。
    pub segment_id: SegmentId,
    /// 所属堆。
    pub heap_id: HeapId,
    /// 所属类型化域。
    pub kind: SegmentKind,
}

impl GraphDomainBlock {
    /// 读取 `u64` 区间（不借 [`GcHeap`]，避免 `RefCell` 重入）。
    ///
    /// 调用方须保证块仍被根 / 钉住保活。
    pub fn read_u64s(&self, offset_elems: usize, len: usize) -> Result<Vec<u64>> {
        if !matches!(self.kind, SegmentKind::GraphIndex | SegmentKind::GraphProperty) {
            return Err(GcError::InvalidCapacity);
        }
        let byte_off = offset_elems.checked_mul(8).ok_or(GcError::InvalidCapacity)?;
        let byte_len = len.checked_mul(8).ok_or(GcError::InvalidCapacity)?;
        let end = byte_off.checked_add(byte_len).ok_or(GcError::InvalidCapacity)?;
        if end > self.byte_len {
            return Err(GcError::InvalidCapacity);
        }
        let mut out = vec![0u64; len];
        // SAFETY: 调用方保证载荷在根/钉住下仍有效；边界已校验。
        unsafe {
            let src = self.ptr.as_ptr().add(byte_off).cast::<u64>();
            core::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), len);
        }
        Ok(out)
    }

    /// 写入 `u64` 区间（不借 [`GcHeap`]）。
    pub fn write_u64s(&self, offset_elems: usize, values: &[u64]) -> Result<()> {
        if !matches!(self.kind, SegmentKind::GraphIndex | SegmentKind::GraphProperty) {
            return Err(GcError::InvalidCapacity);
        }
        let byte_off = offset_elems.checked_mul(8).ok_or(GcError::InvalidCapacity)?;
        let byte_len = values.len().checked_mul(8).ok_or(GcError::InvalidCapacity)?;
        let end = byte_off.checked_add(byte_len).ok_or(GcError::InvalidCapacity)?;
        if end > self.byte_len {
            return Err(GcError::InvalidCapacity);
        }
        // SAFETY: 写路径仅在分配后或 pin 保护下使用；bounds 已校验。
        unsafe {
            let dst = self.ptr.as_ptr().add(byte_off).cast::<u64>();
            core::ptr::copy_nonoverlapping(values.as_ptr(), dst, values.len());
        }
        Ok(())
    }
}

unsafe impl Send for GraphDomainBlock {}

impl GcHeap {
    /// 在 [`SegmentKind::GraphIndex`] 域分配载荷。
    pub fn allocate_graph_index(&mut self, payload_bytes: usize) -> Result<GraphDomainBlock> {
        self.allocate_graph_domain(SegmentKind::GraphIndex, BlockKind::GraphIndex, payload_bytes)
    }

    /// 在 [`SegmentKind::GraphProperty`] 域分配载荷。
    pub fn allocate_graph_property(&mut self, payload_bytes: usize) -> Result<GraphDomainBlock> {
        self.allocate_graph_domain(SegmentKind::GraphProperty, BlockKind::GraphProperty, payload_bytes)
    }

    /// 分配 GraphIndex 域并写入 `u64` 序列（空切片仍分配最小 8 字节槽）。
    pub fn allocate_graph_index_u64s(&mut self, values: &[u64]) -> Result<GraphDomainBlock> {
        let bytes = values.len().saturating_mul(8).max(8);
        let block = self.allocate_graph_index(bytes)?;
        if !values.is_empty() {
            self.write_graph_domain_u64s(&block, 0, values)?;
        }
        Ok(block)
    }

    /// 分配 GraphProperty 域并写入 `u64` 序列。
    pub fn allocate_graph_property_u64s(&mut self, values: &[u64]) -> Result<GraphDomainBlock> {
        let bytes = values.len().saturating_mul(8).max(8);
        let block = self.allocate_graph_property(bytes)?;
        if !values.is_empty() {
            self.write_graph_domain_u64s(&block, 0, values)?;
        }
        Ok(block)
    }

    /// 从 GraphIndex / GraphProperty 块读取 `u64` 区间（按元素下标）。
    pub fn read_graph_domain_u64s(&self, block: &GraphDomainBlock, offset_elems: usize, len: usize) -> Result<Vec<u64>> {
        self.ensure_graph_domain_block(block)?;
        block.read_u64s(offset_elems, len)
    }

    /// 向 GraphIndex / GraphProperty 块写入 `u64` 区间。
    pub fn write_graph_domain_u64s(&mut self, block: &GraphDomainBlock, offset_elems: usize, values: &[u64]) -> Result<()> {
        self.ensure_graph_domain_block(block)?;
        block.write_u64s(offset_elems, values)
    }

    fn allocate_graph_domain(&mut self, kind: SegmentKind, block_kind: BlockKind, payload_bytes: usize) -> Result<GraphDomainBlock> {
        if payload_bytes == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let (segment_id, ptr) = self.allocate_payload(kind, block_kind, payload_bytes, u32::MAX, NumericOwnership::Unspecified)?;
        Ok(GraphDomainBlock { ptr, byte_len: payload_bytes, segment_id, heap_id: self.id, kind })
    }

    fn ensure_graph_domain_block(&self, block: &GraphDomainBlock) -> Result<()> {
        if block.heap_id != self.id {
            return Err(GcError::WrongHeap);
        }
        if !matches!(block.kind, SegmentKind::GraphIndex | SegmentKind::GraphProperty) {
            return Err(GcError::InvalidCapacity);
        }
        Ok(())
    }
}
