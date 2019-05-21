//! 证据接纳门控与候选。

pub mod candidate;
pub mod gate;
pub mod semantic;

pub use candidate::OuterCandidate;
pub use gate::{
    AdmissionGate, AdmissionOutcome, AdmissionRejectReason, EvidenceVerifier, VerificationPolicy, admit_polynomial_exact,
    admit_polynomial_result, is_admitted,
};
pub use semantic::SemanticCore;
