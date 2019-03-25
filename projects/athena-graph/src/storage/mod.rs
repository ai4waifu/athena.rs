//! 图存储层：CSR/CSC、属性列、capability 与转换。

mod capability;
mod conversion;
mod csc;
mod csr;
mod derived_csc;
mod gc_payload;
mod property;

pub use capability::{GraphAlgorithmRequirements, GraphCapabilities};
pub use conversion::{
    attach_csr_chunks, csr_to_csc, edge_list_to_csr, finish_csr_on_heap, graph_edge_list, graph_to_csr, graph_to_csr_on_heap,
    CsrOnHeap,
};
pub use csc::CscGraph;
pub use csr::CsrGraph;
pub use derived_csc::DerivedCsc;
pub use gc_payload::GcPayloadStorage;
pub use property::{GcDenseU64Column, PropertyCell, PropertyColumn, PropertyStore, WeightColumn, WeightDomainTag};
