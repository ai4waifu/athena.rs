//! 图论领域模块（`athena-engine` 内，非独立 crate）。
//!
//! 图论算法调用 [`athena_graph`] 结构原语。数学结论与证书合同在本模块。

mod bipartite;
mod connectivity;
mod lifecycle;
mod mst;
mod object;
mod path;
mod property;
mod request;
mod result;

pub use athena_graph::{GraphId, GraphRevision, GraphSnapshot, RepresentationId};
pub use lifecycle::{GraphResidencyController, bind_algorithm_checkpoint, resume_from_algorithm_checkpoint};
pub use object::{
    GraphAssumptions, GraphDomainSemantics, GraphHandle, GraphNodeId, GraphObject, GraphPresentation, GraphProvenance,
    MemoryGraph, WeightDomain,
};
pub use property::{CertificateStrength, GraphCertificate, GraphPropertyKind, GraphPropertyResult, GraphPropertyState};
pub use request::GraphTheoryRequest;
pub use result::{
    BipartiteResult, ConnectedComponentsResult, GraphTheoryResult, GraphTheoryValue, MinimumSpanningForestResult,
    ShortestPathResult, SpanningEdge, StronglyConnectedComponentsResult, execute_graph_theory, operation_name,
};
