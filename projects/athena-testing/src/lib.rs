//! Athena neutral **test helpers** (Living `12` / `27`).
//!
//! Builders and typed asserts for use from other crates' `cargo test`
//! (`athena-engine`, `athena`, …). This package is a library only — not a
//! standalone test binary and not `@sxo/harness`.

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

#[cfg(test)]
mod contract;

pub use assertions::{assert_exact_integer, assert_structural_eq, expect_diagnostic};
pub use builders::{DomainRequestBuilder, SessionFixture, TermBuilder};
pub use requests::{goal_request, term_request};
