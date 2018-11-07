//! General graph structures. M-Graph semantics remain in `athena-engine`.
#![deny(missing_docs)]
#![forbid(unsafe_code)]
use athena_ndarray::{ArrayError, ChunkStore, ChunkedArray};

/// Stable in-memory node identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);
/// Stable in-memory edge identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(pub usize);
/// Small directed adjacency-list graph.
#[derive(Debug, Clone, Default)]
pub struct Graph<N, E> {
    nodes: Vec<N>,
    edges: Vec<(NodeId, NodeId, E)>,
    outgoing: Vec<Vec<EdgeId>>,
}
impl<N, E> Graph<N, E> {
    /// Creates a graph.
    pub const fn new() -> Self {
        Self { nodes: vec![], edges: vec![], outgoing: vec![] }
    }
    /// Adds a node.
    pub fn add_node(&mut self, value: N) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(value);
        self.outgoing.push(vec![]);
        id
    }
    /// Adds a directed edge.
    pub fn add_edge(&mut self, source: NodeId, target: NodeId, value: E) -> Option<EdgeId> {
        if source.0 >= self.nodes.len() || target.0 >= self.nodes.len() {
            return None;
        }
        let id = EdgeId(self.edges.len());
        self.edges.push((source, target, value));
        self.outgoing[source.0].push(id);
        Some(id)
    }
    /// Iterates outgoing neighbors.
    pub fn neighbors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.outgoing.get(node.0).into_iter().flatten().map(|id| self.edges[id.0].1)
    }
}

/// Storage-backed directed CSR graph using ndarray chunk stores.
#[derive(Debug)]
pub struct CsrGraph<O, I> {
    nodes: u64,
    edges: u64,
    offsets: ChunkedArray<u64, O>,
    indices: ChunkedArray<u64, I>,
}
impl<O: ChunkStore<u64>, I: ChunkStore<u64>> CsrGraph<O, I> {
    /// Creates a graph and validates outer CSR boundaries.
    pub fn new(
        nodes: u64,
        offsets: ChunkedArray<u64, O>,
        indices: ChunkedArray<u64, I>,
    ) -> Result<Self, CsrError<O::Error, I::Error>> {
        let required = nodes.checked_add(1).ok_or(CsrError::NodeOverflow)?;
        if offsets.shape().element_count() != required {
            return Err(CsrError::OffsetLength);
        }
        let edges = indices.shape().element_count();
        let first = offsets.read_range(0, 1).map_err(CsrError::Offsets)?[0];
        let last = offsets.read_range(nodes, 1).map_err(CsrError::Offsets)?[0];
        if first != 0 || last != edges {
            return Err(CsrError::Boundary);
        }
        Ok(Self { nodes, edges, offsets, indices })
    }
    /// Visits outgoing targets in chunks bounded by the indices memory budget.
    pub fn for_each_neighbor_chunk(
        &self,
        node: u64,
        mut visit: impl FnMut(&[u64]),
    ) -> Result<(), CsrError<O::Error, I::Error>> {
        if node >= self.nodes {
            return Err(CsrError::InvalidNode);
        }
        let bounds = self.offsets.read_range(node, 2).map_err(CsrError::Offsets)?;
        if bounds[0] > bounds[1] || bounds[1] > self.edges {
            return Err(CsrError::Boundary);
        }
        let max = self.indices.memory_budget().bytes() / std::mem::size_of::<u64>();
        let mut offset = bounds[0];
        while offset < bounds[1] {
            let len = usize::try_from((bounds[1] - offset).min(max as u64)).unwrap_or(max);
            let chunk = self.indices.read_range(offset, len).map_err(CsrError::Indices)?;
            if chunk.iter().any(|&target| target >= self.nodes) {
                return Err(CsrError::InvalidTarget);
            }
            visit(&chunk);
            offset += len as u64;
        }
        Ok(())
    }
}
/// CSR graph error.
#[derive(Debug)]
pub enum CsrError<O, I> {
    /// Node count overflow.
    NodeOverflow,
    /// Offset length mismatch.
    OffsetLength,
    /// Invalid CSR boundary/range.
    Boundary,
    /// Invalid node.
    InvalidNode,
    /// Invalid target.
    InvalidTarget,
    /// Offset store error.
    Offsets(ArrayError<O>),
    /// Index store error.
    Indices(ArrayError<I>),
}
