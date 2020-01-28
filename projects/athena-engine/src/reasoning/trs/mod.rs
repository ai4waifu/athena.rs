//! Neutral typed TRS pattern contract — **re-exported from [`athena_rewriter`]**.
//!
//! Ownership: `TermPattern` / `match_pattern` / `substitute` live in `athena-rewriter`.
//! This module remains a compatibility facade for engine / request / ExecutionIR paths.

pub use athena_rewriter::{PatternBindings, PatternConstraint, TermPattern, match_pattern, substitute};
