//! 语义算法与统一策略。

pub mod comparison;
pub mod context;
pub mod kernel_number;
pub mod modular_ops;
pub mod modulus_context;
pub mod promotion;
pub mod rounding;

pub use context::{ExecutionBudget, NumericContext};
