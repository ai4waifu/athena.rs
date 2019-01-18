//! 具体表示（不拥有运算调度）。

pub mod decimal;
pub mod domain;
pub mod dyadic;
pub mod polynomial_fingerprint;
pub mod precision;

pub use decimal::{Decimal, RoundingStatus};
