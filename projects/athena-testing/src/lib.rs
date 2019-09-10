//! Athena neutral semantic acceptance (Living `12` / `27`).
//!
//! Cross-crate contract tests for typed Athena requests. No dialect, parser,
//! or named-head construction APIs. Not `@sxo/harness`.

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
