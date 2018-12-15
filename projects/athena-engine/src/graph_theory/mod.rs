//! 图论领域模块（`athena-engine` 内，非独立 crate）。
//!
//! 图论算法调用 [`athena_graph`] 结构原语。数学结论与证书合同在本模块。

mod connectivity;
mod object;
mod path;
mod property;
mod request;
mod result;

pub use object::{
    GraphAssumptions, GraphHandle, GraphNodeId, GraphObject, GraphPresentation, GraphProvenance, GraphSemantics, MemoryGraph,
    WeightDomain,
};
pub use property::{GraphCertificate, GraphPropertyKind, GraphPropertyResult, GraphPropertyState};
pub use request::GraphTheoryRequest;
pub use result::{ConnectedComponentsResult, GraphTheoryResult, GraphTheoryValue, ShortestPathResult, execute_graph_theory};
