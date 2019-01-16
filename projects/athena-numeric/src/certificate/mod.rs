//! 结果证书与 evidence。

mod certificate;
mod evidence;

pub use certificate::{CertificateMethod, NumericCertificate};
#[allow(deprecated)]
pub use evidence::NumericProvenance;
pub use evidence::{NumericBinding, NumericEvidenceArena, NumericEvidenceId, NumericEvidenceRecord};
