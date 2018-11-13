//! 普通离散图结构与大图算法基座。
//!
//! **不是** M-Graph：不含等价类、witness、hyper-edge、closure 或 solver frontier。
//! CSR / 工作区复用 [`athena_ndarray`] 的 storage 与 memory budget 合同。

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod algo;
mod csr;
mod error;
mod graph;
mod id;

pub use algo::{bfs_order, connected_components, topological_sort};
pub use csr::CsrGraph;
pub use error::GraphError;
pub use graph::{Graph, GraphAlgorithmRequirements, GraphDirection, GraphView};
pub use id::{EdgeId, NodeId};
