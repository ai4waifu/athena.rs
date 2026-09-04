//! Typed claim 与 admission journal。

pub mod claim;
pub mod journal;

pub use claim::{Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, VerifiedClaim, proposition_from_cache_key};
pub use journal::{AdmissionJournal, FactId};
