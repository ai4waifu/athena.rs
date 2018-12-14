//! 图表示转换：邻接表 → CSR、CSR → CSC、边列表 → CSR。

use athena_ndarray::{ArrayStorage, ChunkedArray, InMemoryStorage, LogicalShape, MemoryBudget};

use crate::{CscGraph, CsrGraph, EdgeId, Graph, GraphDirection, GraphError, NodeId};

/// 内存邻接表 → CSR（出邻接按目标升序）。
pub fn graph_to_csr<N, E>(
    graph: &Graph<N, E>,
    budget: MemoryBudget,
) -> Result<CsrGraph<InMemoryStorage<u64>, InMemoryStorage<u64>>, GraphError> {
    if graph.direction() != GraphDirection::Directed {
        return Err(GraphError::UndirectedCsr);
    }
    let nodes = graph.node_count();
    let edges = graph.edge_count();
    let mut adjacency = vec![Vec::new(); nodes as usize];
    for u in 0..nodes {
        let source = NodeId(u);
        for target in graph.out_neighbors(source) {
            adjacency[u as usize].push(target.0);
        }
        adjacency[u as usize].sort_unstable();
    }
    let mut offsets = Vec::with_capacity(nodes as usize + 1);
    let mut indices = Vec::with_capacity(edges as usize);
    offsets.push(0);
    for list in &adjacency {
        indices.extend(list);
        offsets.push(indices.len() as u64);
    }
    let offset_shape = LogicalShape::new([offsets.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let index_shape = LogicalShape::new([indices.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let offsets_arr = ChunkedArray::new(offset_shape, InMemoryStorage::from_vec(offsets), budget)?;
    let indices_arr = ChunkedArray::new(index_shape, InMemoryStorage::from_vec(indices), budget)?;
    CsrGraph::new(nodes, offsets_arr, indices_arr)
}

/// 边列表 `(source, target)` → CSR；边按 `(source, target)` 字典序排序后写入。
pub fn edge_list_to_csr(
    nodes: u64,
    mut edges: Vec<(u64, u64)>,
    budget: MemoryBudget,
) -> Result<CsrGraph<InMemoryStorage<u64>, InMemoryStorage<u64>>, GraphError> {
    edges.sort_unstable();
    let mut offsets = vec![0u64; nodes as usize + 1];
    let mut indices = Vec::with_capacity(edges.len());
    let mut cursor = 0usize;
    for source in 0..nodes {
        while cursor < edges.len() && edges[cursor].0 == source {
            let (_, target) = edges[cursor];
            if target >= nodes {
                return Err(GraphError::InvalidTarget);
            }
            indices.push(target);
            cursor += 1;
        }
        offsets[source as usize + 1] = indices.len() as u64;
    }
    if cursor != edges.len() {
        return Err(GraphError::InvalidEdgeList);
    }
    let offset_shape = LogicalShape::new([offsets.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let index_shape = LogicalShape::new([indices.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let offsets_arr = ChunkedArray::new(offset_shape, InMemoryStorage::from_vec(offsets), budget)?;
    let indices_arr = ChunkedArray::new(index_shape, InMemoryStorage::from_vec(indices), budget)?;
    CsrGraph::new(nodes, offsets_arr, indices_arr)
}

/// CSR → CSC（按需物化；读取完整 indices 后转置）。
pub fn csr_to_csc<O: ArrayStorage<u64>, I: ArrayStorage<u64>>(
    csr: &CsrGraph<O, I>,
    budget: MemoryBudget,
) -> Result<CscGraph<InMemoryStorage<u64>, InMemoryStorage<u64>>, GraphError> {
    let nodes = csr.node_count();
    let edges = csr.edge_count();
    let mut column_lists = vec![Vec::new(); nodes as usize];
    for source in 0..nodes {
        let mut collected = Vec::new();
        csr.for_each_neighbor_chunk(source, |chunk| collected.extend_from_slice(chunk))?;
        for &target in &collected {
            column_lists[target as usize].push(source);
        }
    }
    let mut column_offsets = Vec::with_capacity(nodes as usize + 1);
    let mut row_indices = Vec::with_capacity(edges as usize);
    column_offsets.push(0);
    for mut list in column_lists {
        list.sort_unstable();
        row_indices.extend(list);
        column_offsets.push(row_indices.len() as u64);
    }
    let offset_shape = LogicalShape::new([column_offsets.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let index_shape = LogicalShape::new([row_indices.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let column_offsets_arr = ChunkedArray::new(offset_shape, InMemoryStorage::from_vec(column_offsets), budget)?;
    let row_indices_arr = ChunkedArray::new(index_shape, InMemoryStorage::from_vec(row_indices), budget)?;
    CscGraph::new(nodes, column_offsets_arr, row_indices_arr)
}

/// 从 [`Graph`] 导出边列表 `(source, target, edge_id)`。
pub fn graph_edge_list<N, E>(graph: &Graph<N, E>) -> Vec<(NodeId, NodeId, EdgeId)> {
    graph.edges().collect()
}
