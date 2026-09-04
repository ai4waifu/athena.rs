//! Session、语义身份、值与对象表。

pub mod objects;
pub mod results;
pub mod semantic;
pub mod session;
pub mod symbols;
pub mod values;

pub use results::{ComputationResult, CoverageStatus, ResultEvidence, ResultProvenance, ResultProviderId, ResultStore};
pub use session::Session;
pub use values::{RuntimeValue, ValueStore};
