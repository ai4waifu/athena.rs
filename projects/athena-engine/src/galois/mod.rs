//! 伽罗瓦理论 — 扩张性质、自同构、伽罗瓦群（骨架）。
//!
//! 调用 `polynomial` / `field`，不反向拥有其语义，也不复制实现。

mod request;
mod result;
mod value;

pub use request::GaloisRequest;
pub use result::{GaloisResult, execute_galois};
pub use value::{FieldAutomorphism, FieldAutomorphism as Automorphism, GaloisComputation, GaloisDomainValue, GaloisGroup};
