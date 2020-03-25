//! 自 `src/reasoning/mgraph/facts/witness.rs` 迁出的原内联测试。

use athena_types::TermId;

use athena_engine::{
    Session,
    reasoning::mgraph::{
        CapabilityProviderId,
        facts::{claim::EvidenceCertificate, *},
    },
};
use athena_ir::fnv1a64;

#[test]
fn structural_witness_is_stable_and_order_sensitive() {
    let a = Evidence::TrustedKernel {
        provider: CapabilityProviderId(20),
        certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(1), right: TermId(2) },
        summary: "ignored".into(),
    };
    let b = Evidence::TrustedKernel {
        provider: CapabilityProviderId(20),
        certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(1), right: TermId(2) },
        summary: "different summary".into(),
    };
    let c = Evidence::TrustedKernel {
        provider: CapabilityProviderId(20),
        certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(2), right: TermId(1) },
        summary: "ignored".into(),
    };
    assert_eq!(witness_ref_from_evidence(&a), witness_ref_from_evidence(&b));
    assert_ne!(witness_ref_from_evidence(&a), witness_ref_from_evidence(&c));
}
