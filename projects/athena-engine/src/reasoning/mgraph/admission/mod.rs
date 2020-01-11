//! 证据接纳门控与候选。

pub mod candidate;
pub mod gate;
pub mod hyper_edge;
pub mod outer_admit;
pub mod semantic;

pub use candidate::OuterCandidate;
pub use gate::{
    AdmissionGate, AdmissionOutcome, AdmissionRejectReason, CALCULUS_PROVIDER_ID, CONGRUENCE_PROVIDER_ID, EvidenceVerifier,
    VerificationPolicy, admit_polynomial_exact, admit_polynomial_result, is_admitted,
};
pub use hyper_edge::{HYPER_EDGE_STAGING_PROVIDER_ID, hyper_edge_to_outer_candidate};
pub use outer_admit::{OUTER_STRUCTURAL_PROVIDER_ID, OuterAdmitReport, admit_outer_pool_if_structural};
pub use semantic::SemanticCore;
