//! Typed claim 与 admission journal。

pub mod claim;
pub mod journal;
pub mod witness;

pub use claim::{
    CalculusRelationKind, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, VerifiedClaim,
    proposition_from_cache_key,
};
pub use journal::{AdmissionJournal, FactId};
pub use witness::witness_ref_from_evidence;
