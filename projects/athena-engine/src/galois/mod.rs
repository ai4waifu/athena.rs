//! 伽罗瓦理论 — 扩张性质、自同构、伽罗瓦群。
//!
//! 已实现：请求/值对象合同。完整 Galois 群计算分派待扩展。
//!
//! 调用 `polynomial` / `field`，不反向拥有其语义，也不复制实现。

mod compute;
mod request;
mod result;
mod value;

pub use compute::execute_galois_with_tables;
pub use request::GaloisRequest;
pub use result::{GaloisResult, execute_galois};
pub use value::{FieldAutomorphism, FieldAutomorphism as Automorphism, GaloisComputation, GaloisDomainValue, GaloisGroup};
