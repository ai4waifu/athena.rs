//! 普通离散图结构与大图算法基座。
//!
//! **不是** M-Graph：不含等价类、witness、hyper-edge、closure 或 solver frontier。
//! CSR / CSC / 工作区复用 [`athena_ndarray`] 的 storage 与 memory budget 合同。

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod algo;
mod capability;
mod conversion;
mod csc;
mod csr;
mod error;
mod graph;
mod id;
mod view;

pub use algo::{UnionFind, bfs_order, connected_components, strongly_connected_components, topological_sort};
pub use capability::{GraphAlgorithmRequirements, GraphCapabilities};
pub use conversion::{csr_to_csc, edge_list_to_csr, graph_edge_list, graph_to_csr};
pub use csc::CscGraph;
pub use csr::CsrGraph;
pub use error::GraphError;
pub use graph::{Graph, GraphDirection, GraphView};
pub use id::{EdgeId, GraphRevision, NodeId};
pub use view::{EdgeFilteredView, InducedSubgraphView, ReversedGraphView};
