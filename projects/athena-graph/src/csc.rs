//! Storage-backed CSC（按需构建，不默认与 CSR 双物化）。

use athena_ndarray::{ArrayError, ArrayStorage, ChunkedArray};

use crate::{capability::GraphCapabilities, error::GraphError, semantics::GraphStorageMetadata};

/// Storage-backed 有向 CSC 图（列指针 + 行索引）。
#[derive(Debug)]
pub struct CscGraph<O, I> {
    nodes: u64,
    edges: u64,
    column_offsets: ChunkedArray<u64, O>,
    row_indices: ChunkedArray<u64, I>,
    metadata: Option<GraphStorageMetadata>,
}

impl<O: ArrayStorage<u64>, I: ArrayStorage<u64>> CscGraph<O, I> {
    /// 创建并全量校验 CSC invariants（含 offsets 单调）。
    pub fn new(
        nodes: u64,
        column_offsets: ChunkedArray<u64, O>,
        row_indices: ChunkedArray<u64, I>,
    ) -> Result<Self, GraphError> {
        Self::new_with_metadata(nodes, column_offsets, row_indices, None)
    }

    /// 创建并附带存储元数据。
    pub fn new_with_metadata(
        nodes: u64,
        column_offsets: ChunkedArray<u64, O>,
        row_indices: ChunkedArray<u64, I>,
        metadata: Option<GraphStorageMetadata>,
    ) -> Result<Self, GraphError> {
        let required = nodes.checked_add(1).ok_or(GraphError::NodeOverflow)?;
        if column_offsets.shape().element_count() != required {
            return Err(GraphError::OffsetLength);
        }
        let edges = row_indices.shape().element_count();
        validate_column_offsets_monotonic(&column_offsets, nodes, edges)?;
        Ok(Self { nodes, edges, column_offsets, row_indices, metadata })
    }

    /// 绑定 / 替换元数据。
    pub fn set_metadata(&mut self, metadata: GraphStorageMetadata) {
        self.metadata = Some(metadata);
    }

    /// 存储元数据。
    pub fn metadata(&self) -> Option<&GraphStorageMetadata> {
        self.metadata.as_ref()
    }

    /// 节点数。
    pub const fn node_count(&self) -> u64 {
        self.nodes
    }

    /// 边数。
    pub const fn edge_count(&self) -> u64 {
        self.edges
    }

    /// 按 row_indices memory budget 分块访问入邻接。
    pub fn for_each_in_neighbor_chunk(&self, column: u64, mut visit: impl FnMut(&[u64])) -> Result<(), GraphError> {
        if column >= self.nodes {
            return Err(GraphError::InvalidNode);
        }
        let bounds = self.column_offsets.read_range(column, 2)?;
        if bounds[0] > bounds[1] || bounds[1] > self.edges {
            return Err(GraphError::Boundary);
        }
        let max = self.row_indices.memory_budget().bytes() / std::mem::size_of::<u64>();
        if max == 0 {
            return Err(GraphError::Array(ArrayError::BudgetTooSmall { element_size: std::mem::size_of::<u64>() }));
        }
        let mut offset = bounds[0];
        while offset < bounds[1] {
            let remaining = bounds[1] - offset;
            let len = usize::try_from(remaining.min(max as u64)).unwrap_or(max);
            let chunk = self.row_indices.read_range(offset, len)?;
            if chunk.iter().any(|&row| row >= self.nodes) {
                return Err(GraphError::InvalidTarget);
            }
            visit(&chunk);
            offset = offset.checked_add(len as u64).ok_or(GraphError::Array(ArrayError::RangeOverflow))?;
        }
        Ok(())
    }

    /// capability 报告。
    pub fn capabilities(&self) -> GraphCapabilities {
        GraphCapabilities {
            in_memory: false,
            sorted_adjacency: true,
            reverse_adjacency: true,
            random_access: true,
            chunked_sequential: true,
            external_workspace: true,
        }
    }
}

fn validate_column_offsets_monotonic<O: ArrayStorage<u64>>(
    offsets: &ChunkedArray<u64, O>,
    nodes: u64,
    edges: u64,
) -> Result<(), GraphError> {
    let first = offsets.read_range(0, 1)?[0];
    if first != 0 {
        return Err(GraphError::Boundary);
    }
    let mut prev = 0u64;
    let mut i = 0u64;
    while i <= nodes {
        let cur = offsets.read_range(i, 1)?[0];
        if cur < prev || cur > edges {
            return Err(GraphError::OffsetNonMonotonic { index: i, prev, cur });
        }
        prev = cur;
        i += 1;
    }
    if prev != edges {
        return Err(GraphError::Boundary);
    }
    Ok(())
}
