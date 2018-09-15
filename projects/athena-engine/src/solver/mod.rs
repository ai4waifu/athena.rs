//! 求解器执行协议（骨架）— 属 `athena-engine` 内部，禁止 `athena-solver` crate。

mod frontier;
mod reflector;
mod registry;
mod request;
mod types;

pub use frontier::score_candidate;
pub use reflector::{ReflectionResult, Reflector, SolverContext};
pub use registry::SolverRegistry;
pub use request::{DomainRef, SolverLimits, SolverOperation, SolverRequest};
pub use types::{SolverId, SolverMetadata};
