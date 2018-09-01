//! 面向 Core IR 的改写器（规则、规范化、化简）。

#![deny(missing_docs)]

mod engine;

pub use engine::{RewriteOptions, RewriteResult, Rewriter};
