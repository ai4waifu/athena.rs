//! 图表示转换：邻接表 → CSR、CSR → CSC、边列表 → CSR。

use athena_gc::GcHeap;
use athena_ndarray::{ArrayStorage, ChunkedArray, InMemoryStorage, LogicalShape, MemoryBudget};

use crate::{
    CscGraph, CsrGraph, EdgeId, GraphDirection, GraphError, MutableGraph, NodeId,
    lifecycle::{
        ChunkRegistry, ChunkSet, GraphPublication, PublishedImmutableGraph, allocate_chunk_id, finish_on_heap, publication_attach_chunks,
    },
    storage::gc_payload::GcPayloadStorage,
};

/// CSR 物化结果（offsets / indices 在 GraphIndex segment）+ 登记的 [`ChunkSet`]。
#[derive(Debug)]
pub struct CsrOnHeap {
    /// CSR（GC payload storage）。
    pub csr: CsrGraph<GcPayloadStorage, GcPayloadStorage>,
    /// offsets + indices 两个 chunk。
    pub chunks: ChunkSet,
}

/// 内存邻接表 → CSR（出邻接按目标升序）。
pub fn graph_to_csr<N, E>(
    graph: &MutableGraph<N, E>,
    budget: MemoryBudget,
) -> Result<CsrGraph<InMemoryStorage<u64>, InMemoryStorage<u64>>, GraphError> {
    let (offsets, indices, nodes) = build_csr_vecs(graph)?;
    let offset_shape = LogicalShape::new([offsets.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let index_shape = LogicalShape::new([indices.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let offsets_arr = ChunkedArray::new(offset_shape, InMemoryStorage::from_vec(offsets), budget)?;
    let indices_arr = ChunkedArray::new(index_shape, InMemoryStorage::from_vec(indices), budget)?;
    let meta = crate::GraphStorageMetadata::csr_unbound(true).bind_snapshot(crate::GraphSnapshot::new(
        graph.id(),
        graph.revision(),
        graph.semantics(),
        crate::RepresentationId::CSR,
    ));
    CsrGraph::new_with_metadata(nodes, offsets_arr, indices_arr, Some(meta))
}

/// 邻接表 → CSR，offsets/indices 写入 GraphIndex segment，并登记 chunk。
pub fn graph_to_csr_on_heap<N, E>(
    graph: &MutableGraph<N, E>,
    heap: &mut GcHeap,
    registry: &mut ChunkRegistry,
    budget: MemoryBudget,
) -> Result<CsrOnHeap, GraphError> {
    let (offsets, indices, nodes) = build_csr_vecs(graph)?;
    let offsets_chunk = allocate_chunk_id(heap)?;
    let indices_chunk = allocate_chunk_id(heap)?;
    registry.register_resident(offsets_chunk)?;
    registry.register_resident(indices_chunk)?;
    let offsets_store = GcPayloadStorage::allocate_index(heap, &offsets, offsets_chunk)?;
    let indices_store = GcPayloadStorage::allocate_index(heap, &indices, indices_chunk)?;
    let offset_shape = LogicalShape::new([offsets.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let index_shape = LogicalShape::new([indices.len() as u64]).map_err(|_| GraphError::NodeOverflow)?;
    let offsets_arr = ChunkedArray::new(offset_shape, offsets_store, budget)?;
    let indices_arr = ChunkedArray::new(index_shape, indices_store, budget)?;
    let meta = crate::GraphStorageMetadata::csr_unbound(true).bind_snapshot(crate::GraphSnapshot::new(
        graph.id(),
        graph.revision(),
        graph.semantics(),
        crate::RepresentationId::CSR,
    ));
    let csr = CsrGraph::new_with_metadata(nodes, offsets_arr, indices_arr, Some(meta))?;
    let mut chunks = ChunkSet::new();
    chunks.push(offsets_chunk);
    chunks.push(indices_chunk);
    Ok(CsrOnHeap { csr, chunks })
}

/// 将 [`CsrOnHeap::chunks`] 挂到已发布 snapshot（登记 chunk object roots）。
pub fn attach_csr_chunks(heap: &mut GcHeap, publication: &mut GraphPublication, csr: &CsrOnHeap) {
    publication_attach_chunks(heap, publication, csr.chunks.owning_copy());
}

/// `finish()` 正式路径：immutable snapshot + GraphIndex CSR chunks。
pub fn finish_csr_on_heap<N, E>(
    graph: MutableGraph<N, E>,
    heap: &mut GcHeap,
    registry: &mut ChunkRegistry,
    budget: MemoryBudget,
) -> Result<(PublishedImmutableGraph<N, E>, CsrOnHeap), GraphError> {
    let csr = graph_to_csr_on_heap(&graph, heap, registry, budget)?;
    let mut published = finish_on_heap(graph, heap)?;
    attach_csr_chunks(heap, &mut published.publication, &csr);
    Ok((published, csr))
}

fn build_csr_vecs<N, E>(graph: &MutableGraph<N, E>) -> Result<(Vec<u64>, Vec<u64>, u64), GraphError> {
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
    Ok((offsets, indices, nodes))
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
///
/// 若 CSR 带 metadata，CSC 继承 `graph_id`/`revision`/`semantics` 并标记为 [`RepresentationId::CSC`]。
/// 长期缓存请用 [`crate::DerivedCsc`]，以便在源 revision 变更后显式失效。
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
    let metadata = csr.metadata().map(|m| {
        let mut meta = crate::GraphStorageMetadata {
            representation_id: crate::RepresentationId::CSC,
            graph_id: m.graph_id,
            revision: m.revision,
            semantics: m.semantics,
            sorted_adjacency: m.sorted_adjacency,
            allows_duplicate_targets: m.allows_duplicate_targets,
        };
        if let (Some(gid), Some(rev), Some(sem)) = (m.graph_id, m.revision, m.semantics) {
            meta = meta.bind_snapshot(crate::GraphSnapshot::new(gid, rev, sem, crate::RepresentationId::CSC));
        }
        meta
    });
    CscGraph::new_with_metadata(nodes, column_offsets_arr, row_indices_arr, metadata)
}

/// 从 [`MutableGraph`] 导出边列表 `(source, target, edge_id)`。
pub fn graph_edge_list<N, E>(graph: &MutableGraph<N, E>) -> Vec<(NodeId, NodeId, EdgeId)> {
    graph.edges().collect()
}
