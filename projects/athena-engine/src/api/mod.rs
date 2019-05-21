//! 对外引擎 API（句柄与选项）。
//!
//! 实现放在私有 `engine` 子模块，避免 `api::engine::…` 与 `api::…` 双路径。

mod engine;

pub use engine::{AthenaEngine, EvalOptions, SimplifyOptions};
