//! 伽罗瓦理论 — 扩张性质、自同构、伽罗瓦群。
//!
//! 已实现：`$\mathbb{F}_{p^n}/\mathbb{F}_p$` 上可分/正规/`IsGalois`、Frobenius 自同构，
//! 以及经 [`execute_galois_with_tables`] 的完整伽罗瓦群（循环 `$C_n$`）。
//! 多项式入口 / 固定域仍 `Unevaluated`。
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
