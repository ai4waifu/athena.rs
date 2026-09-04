//! 求解器执行协议（调度骨架）。
//!
//! 本模块是 planner → provider 的执行请求协议（Reflector / Registry / Frontier /
//! [`SolverRequest`]），**不是** Solve 数学对象层。
//!
//! 跨域约束、goal、解集与覆盖语义见 [`crate::domains::solve`]。
//! 禁止把 [`SolverRequest`] 扩展成 [`crate::domains::solve::SolveProblem`]，也禁止新增
//! `athena-solver` crate。

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
