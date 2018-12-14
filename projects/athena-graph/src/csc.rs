//! Storage-backed CSC（按需构建，不默认与 CSR 双物化）。

use athena_ndarray::{ArrayError, ArrayStorage, ChunkedArray};

use crate::{GraphError, capability::GraphCapabilities};

/// Storage-backed 有向 CSC 图（列指针 + 行索引）。
#[derive(Debug)]
pub struct CscGraph<O, I> {
    nodes: u64,
    edges: u64,
    column_offsets: ChunkedArray<u64, O>,
    row_indices: ChunkedArray<u64, I>,
}

impl<O: ArrayStorage<u64>, I: ArrayStorage<u64>> CscGraph<O, I> {
    /// 创建并校验 CSC 外边界。
    pub fn new(
        nodes: u64,
        column_offsets: ChunkedArray<u64, O>,
        row_indices: ChunkedArray<u64, I>,
    ) -> Result<Self, GraphError> {
        let required = nodes.checked_add(1).ok_or(GraphError::NodeOverflow)?;
        if column_offsets.shape().element_count() != required {
            return Err(GraphError::OffsetLength);
        }
        let edges = row_indices.shape().element_count();
        let first = column_offsets.read_range(0, 1)?[0];
        let last = column_offsets.read_range(nodes, 1)?[0];
        if first != 0 || last != edges {
            return Err(GraphError::Boundary);
        }
        Ok(Self { nodes, edges, column_offsets, row_indices })
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
