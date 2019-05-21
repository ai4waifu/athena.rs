//! Typed claim 与 fact log。

pub mod claim;
pub mod log;

pub use claim::{Claim, Evidence, Guarantee, Proposition, Scope, VerifiedClaim, proposition_from_cache_key};
pub use log::{FactId, FactLog};
