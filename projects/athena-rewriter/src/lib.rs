//! Rewriter over Core IR (rules, canonicalization, simplification).

#![deny(missing_docs)]

mod engine;

pub use engine::{RewriteOptions, RewriteResult, Rewriter};
