//! Athena 中立 **测试辅助**。
//!
//! 供其他 crate 的 `cargo test` 使用的构造器与类型化断言
//! （`athena-engine`、`athena`、…）。本包仅为库 — 不是
//! 独立测试二进制，也不是 `@sxo/harness`。

#![deny(missing_docs)]

pub mod assertions;
pub mod backend;
pub mod builders;
pub mod domains;
pub mod execution;
pub mod fixtures;
pub mod generators;
pub mod lifecycle;
pub mod mgraph;
pub mod requests;
pub mod results;
pub mod rewrite;
pub mod terms;
pub mod values;

pub use assertions::{assert_exact_integer, assert_structural_eq, expect_diagnostic};
pub use builders::{DomainRequestBuilder, SessionFixture, TermBuilder};
pub use requests::{goal_request, term_request};
