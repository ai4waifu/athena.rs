//! 中立 typed TRS 模式合同 — 从 [`athena_rewriter`] 再导出。
//!
//! 所有权：`TermPattern` / `match_pattern` / `substitute` 住在 `athena-rewriter`。
//! 本模块只作 engine / request / `ExecutionIR` 路径的兼容门面。

pub use athena_rewriter::{PatternBindings, PatternConstraint, TermPattern, match_pattern, substitute};
