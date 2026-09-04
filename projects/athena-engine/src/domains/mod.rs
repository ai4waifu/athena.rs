//! 数学领域 providers 与域分派。

pub mod algebra;
pub mod calculus;
pub mod dispatch;
pub mod field;
pub mod galois;
pub mod graph_theory;
pub mod group;
pub mod linear_algebra;
pub mod number_theory;
pub mod optimization;
pub mod polynomial;
pub mod solve;

pub use dispatch::{DomainRequest, DomainResult, execute_domain};
