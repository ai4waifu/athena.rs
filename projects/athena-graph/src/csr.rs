//! Storage-backed CSR。

use athena_ndarray::{ArrayError, ArrayStorage, ChunkedArray};

use crate::GraphError;

/// Storage-backed 有向 CSR 图。
#[derive(Debug)]
pub struct CsrGraph<O, I> {
    nodes: u64,
    edges: u64,
    offsets: ChunkedArray<u64, O>,
    indices: ChunkedArray<u64, I>,
}

impl<O: ArrayStorage<u64>, I: ArrayStorage<u64>> CsrGraph<O, I> {
    /// 创建并校验 CSR 外边界。
    pub fn new(
        nodes: u64,
        offsets: ChunkedArray<u64, O>,
        indices: ChunkedArray<u64, I>,
    ) -> Result<Self, GraphError> {
        let required = nodes.checked_add(1).ok_or(GraphError::NodeOverflow)?;
        if offsets.shape().element_count() != required {
            return Err(GraphError::OffsetLength);
        }
        let edges = indices.shape().element_count();
        let first = offsets.read_range(0, 1)?[0];
        let last = offsets.read_range(nodes, 1)?[0];
        if first != 0 || last != edges {
            return Err(GraphError::Boundary);
        }
        Ok(Self {
            nodes,
            edges,
            offsets,
            indices,
        })
    }

    /// 节点数。
    pub const fn node_count(&self) -> u64 {
        self.nodes
    }

    /// 边数。
    pub const fn edge_count(&self) -> u64 {
        self.edges
    }

    /// 按 indices memory budget 分块访问出邻接。
    pub fn for_each_neighbor_chunk(
        &self,
        node: u64,
        mut visit: impl FnMut(&[u64]),
    ) -> Result<(), GraphError> {
        if node >= self.nodes {
            return Err(GraphError::InvalidNode);
        }
        let bounds = self.offsets.read_range(node, 2)?;
        if bounds[0] > bounds[1] || bounds[1] > self.edges {
            return Err(GraphError::Boundary);
        }
        let max = self.indices.memory_budget().bytes() / std::mem::size_of::<u64>();
        if max == 0 {
            return Err(GraphError::Array(ArrayError::BudgetTooSmall {
                element_size: std::mem::size_of::<u64>(),
            }));
        }
        let mut offset = bounds[0];
        while offset < bounds[1] {
            let remaining = bounds[1] - offset;
            let len = usize::try_from(remaining.min(max as u64)).unwrap_or(max);
            let chunk = self.indices.read_range(offset, len)?;
            if chunk.iter().any(|&target| target >= self.nodes) {
                return Err(GraphError::InvalidTarget);
            }
            visit(&chunk);
            offset = offset
                .checked_add(len as u64)
                .ok_or(GraphError::Array(ArrayError::RangeOverflow))?;
        }
        Ok(())
    }
}
