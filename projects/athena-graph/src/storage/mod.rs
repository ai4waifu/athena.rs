//! 图存储层：CSR/CSC、属性列、capability 与转换。

mod capability;
mod conversion;
mod csc;
mod csr;
mod derived_csc;
mod property;

pub use capability::{GraphAlgorithmRequirements, GraphCapabilities};
pub use conversion::{csr_to_csc, edge_list_to_csr, graph_edge_list, graph_to_csr};
pub use csc::CscGraph;
pub use csr::CsrGraph;
pub use derived_csc::DerivedCsc;
pub use property::{PropertyCell, PropertyColumn, PropertyStore, WeightColumn, WeightDomainTag};
