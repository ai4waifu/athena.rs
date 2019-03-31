//! 经 `athena-gc` GraphIndex / GraphProperty segment 的 `u64` payload storage。

use athena_gc::{GcHeap, GraphDomainBlock, HeapId, RootKind, RootToken, SegmentKind, with_registered_heap};
use athena_ndarray::{ArrayError, ArrayStorage, StorageCapabilities};

use crate::{GraphError, lifecycle::GraphChunkId};

fn map_gc_read(err: athena_gc::GcError) -> ArrayError {
    let _ = err;
    ArrayError::Store
}

/// GraphIndex / GraphProperty 域上的 `u64` 列（实现 [`ArrayStorage`]，图侧无 unsafe）。
#[derive(Debug)]
pub struct GcPayloadStorage {
    heap_id: HeapId,
    block: GraphDomainBlock,
    /// 逻辑元素个数（可小于 block 物理容量）。
    len: u64,
    /// 保活 payload 的 numeric-style root（[`RootKind::Graph`]）。
    payload_root: RootToken,
    /// 对应 lifecycle chunk 身份。
    chunk_id: GraphChunkId,
}

impl GcPayloadStorage {
    /// 在 GraphIndex 域分配并拷入 `values`。
    pub fn allocate_index(heap: &mut GcHeap, values: &[u64], chunk_id: GraphChunkId) -> Result<Self, GraphError> {
        Self::allocate(heap, values, chunk_id, SegmentKind::GraphIndex)
    }

    /// 在 GraphProperty 域分配并拷入 `values`。
    pub fn allocate_property(heap: &mut GcHeap, values: &[u64], chunk_id: GraphChunkId) -> Result<Self, GraphError> {
        Self::allocate(heap, values, chunk_id, SegmentKind::GraphProperty)
    }

    fn allocate(heap: &mut GcHeap, values: &[u64], chunk_id: GraphChunkId, kind: SegmentKind) -> Result<Self, GraphError> {
        let block = match kind {
            SegmentKind::GraphIndex => heap.allocate_graph_index_u64s(values).map_err(GraphError::from)?,
            SegmentKind::GraphProperty => heap.allocate_graph_property_u64s(values).map_err(GraphError::from)?,
            _ => return Err(GraphError::Gc(athena_gc::GcError::InvalidCapacity)),
        };
        let payload_root = heap.roots_mut().register_numeric(block.ptr, RootKind::Graph);
        Ok(Self { heap_id: heap.id(), block, len: values.len() as u64, payload_root, chunk_id })
    }

    /// Lifecycle chunk 身份。
    pub const fn chunk_id(&self) -> GraphChunkId {
        self.chunk_id
    }

    /// Payload root（Session 注销登记时使用）。
    pub const fn payload_root(&self) -> RootToken {
        self.payload_root
    }

    /// 底层 block（只读元数据）。
    pub const fn block(&self) -> GraphDomainBlock {
        self.block
    }

    /// Owner heap。
    pub const fn heap_id(&self) -> HeapId {
        self.heap_id
    }

    /// 逻辑元素个数。
    pub const fn element_count(&self) -> u64 {
        self.len
    }
}

impl Drop for GcPayloadStorage {
    fn drop(&mut self) {
        let _ = with_registered_heap(self.heap_id, |heap| {
            let _ = heap.roots_mut().unregister_numeric(self.payload_root);
            Ok(())
        });
    }
}

impl ArrayStorage<u64> for GcPayloadStorage {
    type Error = ArrayError;

    fn len(&self) -> u64 {
        self.len
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities { writable: true, random_read: true, sequential_read: true, persistent: false }
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<u64>, Self::Error> {
        let start = usize::try_from(offset).map_err(|_| ArrayError::RangeOverflow)?;
        let end = start.checked_add(len).ok_or(ArrayError::RangeOverflow)?;
        if end as u64 > self.len {
            return Err(ArrayError::OutOfBounds);
        }
        // 经 [`GraphDomainBlock::read_u64s`]：不回借 `GcHeap`，避免与外层 `RefCell` 重入。
        self.block.read_u64s(start, len).map_err(map_gc_read)
    }

    fn write_range(&mut self, offset: u64, values: &[u64]) -> Result<(), Self::Error> {
        let start = usize::try_from(offset).map_err(|_| ArrayError::RangeOverflow)?;
        let end = start.checked_add(values.len()).ok_or(ArrayError::RangeOverflow)?;
        if end as u64 > self.len {
            return Err(ArrayError::OutOfBounds);
        }
        self.block.write_u64s(start, values).map_err(map_gc_read)
    }
}
