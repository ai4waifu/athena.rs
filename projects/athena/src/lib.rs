//! Athena 公共 Rust 门面 — 对执行引擎与底层合同 crate 的稳定入口。
//!
//! ```text
//! athena-types → athena-numeric → athena-ir → athena-rewriter → athena-engine → athena
//! ```
//!
//! 本 crate **不**自行实现求值或 Session。每个公开模块只从**拥有方**再导出一次：
//! - 引擎能力 → [`athena_engine`] 对应模块
//! - IR / types / numeric / rewriter → 各自真相源 crate（禁止经 engine 别名转手）
//!
//! 根级只保留 [`AthenaEngine`] / [`Session`]。

#![deny(missing_docs)]

/// 引擎句柄与选项。
pub use athena_engine::api;
/// 诊断构造与表达式摘要。
pub use athena_engine::diagnostics;
/// 数学领域 providers 与顶层域分派。
pub use athena_engine::domains;
/// KernelIR 编译、VM 与 builtins。
pub use athena_engine::execution;
/// 采样合同（非方言 render）。
pub use athena_engine::plot;
/// 改写编排、M-Graph 与 solver 调度。
pub use athena_engine::reasoning;
/// Session、语义表、值与对象。
pub use athena_engine::runtime;

/// AthenaIR（真相源：`athena-ir`）。
pub use athena_ir as ir;
/// 数值塔（真相源：`athena-numeric`）。
pub use athena_numeric as numeric;
/// 改写器（真相源：`athena-rewriter`）。
pub use athena_rewriter as rewriter;
/// 共享身份 / 诊断 / wire 合同（真相源：`athena-types`）。
pub use athena_types as types;

pub use athena_engine::{AthenaEngine, EvalOptions, Session, SimplifyOptions};
