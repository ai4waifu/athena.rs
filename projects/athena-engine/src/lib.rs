//! Athena CAS 执行引擎 — 唯一决定「怎么算」的地方。
//!
//! ```text
//! athena-types → athena-numeric → athena-ir → athena-rewriter → athena-engine → athena
//! ```
//!
//! 本 crate 拥有求值、Session、M-Graph、solver、改写编排、域分派与 `ATHENA_*` 诊断。
//! 不解析方言、不渲染字符串、也不绑定 N-API/WASM。
//!
//! 根级只保留模块树与极薄入口（[`AthenaEngine`] / [`Session`]）。`athena-types` /
//! `athena-numeric` / `athena-ir` / `athena-rewriter` **不**在此再导出 — 由依赖方或
//! `athena` facade 直接引用真相源 crate。

#![deny(missing_docs)]

pub mod api;
pub mod diagnostics;
pub mod domains;
pub mod execution;
pub mod plot;
pub mod reasoning;
pub mod runtime;

pub use api::{AthenaEngine, EvalOptions, SimplifyOptions};
pub use runtime::Session;
