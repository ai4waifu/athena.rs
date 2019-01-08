//! Storage-backed CSR。

use athena_ndarray::{ArrayError, ArrayStorage, ChunkedArray};

use crate::{capability::GraphCapabilities, error::GraphError, semantics::GraphStorageMetadata};

/// Storage-backed 有向 CSR 图。
#[derive(Debug)]
pub struct CsrGraph<O, I> {
    nodes: u64,
    edges: u64,
    offsets: ChunkedArray<u64, O>,
    indices: ChunkedArray<u64, I>,
    metadata: Option<GraphStorageMetadata>,
}

impl<O: ArrayStorage<u64>, I: ArrayStorage<u64>> CsrGraph<O, I> {
    /// 创建并全量校验 CSR invariants（含 offsets 单调）。
    pub fn new(nodes: u64, offsets: ChunkedArray<u64, O>, indices: ChunkedArray<u64, I>) -> Result<Self, GraphError> {
        Self::new_with_metadata(nodes, offsets, indices, None)
    }

    /// 创建并附带存储元数据。
    pub fn new_with_metadata(
        nodes: u64,
        offsets: ChunkedArray<u64, O>,
        indices: ChunkedArray<u64, I>,
        metadata: Option<GraphStorageMetadata>,
    ) -> Result<Self, GraphError> {
        let required = nodes.checked_add(1).ok_or(GraphError::NodeOverflow)?;
        if offsets.shape().element_count() != required {
            return Err(GraphError::OffsetLength);
        }
        let edges = indices.shape().element_count();
        validate_offsets_monotonic(&offsets, nodes, edges)?;
        if let Some(meta) = &metadata {
            if meta.sorted_adjacency {
                validate_sorted_adjacency(&offsets, &indices, nodes)?;
            }
        }
        Ok(Self { nodes, edges, offsets, indices, metadata })
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

    /// 按 indices memory budget 分块访问出邻接。
    pub fn for_each_neighbor_chunk(&self, node: u64, mut visit: impl FnMut(&[u64])) -> Result<(), GraphError> {
        if node >= self.nodes {
            return Err(GraphError::InvalidNode);
        }
        let bounds = self.offsets.read_range(node, 2)?;
        if bounds[0] > bounds[1] || bounds[1] > self.edges {
            return Err(GraphError::Boundary);
        }
        let max = self.indices.memory_budget().bytes() / std::mem::size_of::<u64>();
        if max == 0 {
            return Err(GraphError::Array(ArrayError::BudgetTooSmall { element_size: std::mem::size_of::<u64>() }));
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
            offset = offset.checked_add(len as u64).ok_or(GraphError::Array(ArrayError::RangeOverflow))?;
        }
        Ok(())
    }

    /// capability 报告。
    pub fn capabilities(&self) -> GraphCapabilities {
        let sorted = self.metadata.as_ref().map(|m| m.sorted_adjacency).unwrap_or(true);
        GraphCapabilities {
            in_memory: false,
            sorted_adjacency: sorted,
            reverse_adjacency: false,
            random_access: true,
            chunked_sequential: true,
            external_workspace: true,
            distributed_shards: false,
        }
    }

    /// 校验算法需求；不满足则 [`GraphError::CapabilityMismatch`]（禁止偷偷物化）。
    pub fn ensure_capabilities(&self, req: crate::GraphAlgorithmRequirements) -> Result<(), GraphError> {
        if self.capabilities().satisfies(req) {
            Ok(())
        }
        else {
            Err(GraphError::CapabilityMismatch { requirement: req })
        }
    }
}

fn validate_offsets_monotonic<O: ArrayStorage<u64>>(
    offsets: &ChunkedArray<u64, O>,
    nodes: u64,
    edges: u64,
) -> Result<(), GraphError> {
    let first = offsets.read_range(0, 1)?[0];
    if first != 0 {
        return Err(GraphError::Boundary);
    }
    let mut prev = 0u64;
    // 逐段读取，避免一次性要求 offsets 全进内存。
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

fn validate_sorted_adjacency<O: ArrayStorage<u64>, I: ArrayStorage<u64>>(
    offsets: &ChunkedArray<u64, O>,
    indices: &ChunkedArray<u64, I>,
    nodes: u64,
) -> Result<(), GraphError> {
    for node in 0..nodes {
        let bounds = offsets.read_range(node, 2)?;
        let start = bounds[0];
        let end = bounds[1];
        if start == end {
            continue;
        }
        let mut prev = indices.read_range(start, 1)?[0];
        let mut off = start + 1;
        while off < end {
            let cur = indices.read_range(off, 1)?[0];
            if cur < prev {
                return Err(GraphError::AdjacencyUnsorted { node, offset: off });
            }
            prev = cur;
            off += 1;
        }
    }
    Ok(())
}
