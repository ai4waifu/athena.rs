//! Stable [`WitnessRef`] derivation from admitted evidence (Living `26`).
//!
//! `WitnessRef` indexes the machine-readable certificate — never the display `summary`.

use athena_ir::fnv1a64;

use crate::reasoning::mgraph::{
    core::{refs::WitnessRef, types::CapabilityProviderId},
    facts::claim::{CalculusRelationKind, Evidence, EvidenceCertificate, Guarantee},
};

/// Derive a stable witness identity from trusted-kernel evidence.
pub fn witness_ref_from_evidence(evidence: &Evidence) -> Option<WitnessRef> {
    match evidence {
        Evidence::TrustedKernel {
            provider,
            certificate,
            summary: _,
        } => Some(WitnessRef(hash_certificate(*provider, certificate))),
    }
}

fn hash_certificate(provider: CapabilityProviderId, certificate: &EvidenceCertificate) -> u64 {
    let mut state = fnv1a64(b"athena.witness.v1");
    mix_u64(&mut state, u64::from(provider.0));
    match certificate {
        EvidenceCertificate::PolynomialExact {
            operation,
            request_fingerprint,
            input_hashes,
            groebner_steps,
        } => {
            mix_tag(&mut state, b"poly");
            mix_tag(&mut state, operation.as_str().as_bytes());
            mix_u64(&mut state, *request_fingerprint);
            for h in input_hashes {
                mix_u64(&mut state, *h);
            }
            mix_u64(&mut state, u64::from(groebner_steps.unwrap_or(u32::MAX)));
        }
        EvidenceCertificate::Rejected { guarantee } => {
            mix_tag(&mut state, b"rejected");
            mix_u64(&mut state, guarantee_tag(*guarantee));
        }
        EvidenceCertificate::TestHarness => {
            mix_tag(&mut state, b"test");
        }
        EvidenceCertificate::CalculusExact {
            kind,
            expression_fingerprint,
            variable_fingerprint,
            result_term,
        } => {
            mix_tag(&mut state, b"calculus");
            mix_u64(&mut state, calculus_kind_tag(*kind));
            mix_u64(&mut state, *expression_fingerprint);
            mix_u64(&mut state, *variable_fingerprint);
            mix_u64(&mut state, u64::from(result_term.0));
        }
        EvidenceCertificate::StructuralTermEquality { left, right } => {
            mix_tag(&mut state, b"structural");
            mix_u64(&mut state, u64::from(left.0));
            mix_u64(&mut state, u64::from(right.0));
        }
        EvidenceCertificate::CongruenceExact {
            modulus_fingerprint,
            left,
            right,
        } => {
            mix_tag(&mut state, b"congruence");
            mix_u64(&mut state, *modulus_fingerprint);
            mix_u64(&mut state, *left);
            mix_u64(&mut state, *right);
        }
        EvidenceCertificate::ApplicationCongruence { left, right } => {
            mix_tag(&mut state, b"app-cong");
            mix_u64(&mut state, u64::from(left.0));
            mix_u64(&mut state, u64::from(right.0));
        }
        EvidenceCertificate::TypedRewriteReplay { rule, left, right } => {
            mix_tag(&mut state, b"typed-rewrite");
            mix_u64(&mut state, u64::from(rule.0));
            mix_u64(&mut state, u64::from(left.0));
            mix_u64(&mut state, u64::from(right.0));
        }
    }
    state
}

fn mix_tag(state: &mut u64, tag: &[u8]) {
    *state ^= fnv1a64(tag);
    *state = state.wrapping_mul(0x0000_0100_0000_01b3);
}

fn mix_u64(state: &mut u64, v: u64) {
    *state ^= v;
    *state = state.wrapping_mul(0x0000_0100_0000_01b3);
}

fn guarantee_tag(g: Guarantee) -> u64 {
    match g {
        Guarantee::ProvenExact => 1,
        Guarantee::ConditionalExact => 2,
        Guarantee::CertifiedApproximation => 3,
        Guarantee::Probable => 4,
        Guarantee::Partial => 5,
        Guarantee::LowerBound => 6,
        Guarantee::UpperBound => 7,
        Guarantee::Candidate => 8,
        Guarantee::Unknown => 9,
    }
}

fn calculus_kind_tag(kind: CalculusRelationKind) -> u64 {
    match kind {
        CalculusRelationKind::DerivativeOf => 1,
        CalculusRelationKind::IntegralOf => 2,
        CalculusRelationKind::SeriesExpansion => 3,
    }
}

#[cfg(test)]
mod tests {
    use athena_types::TermId;

    use super::*;
    use crate::reasoning::mgraph::facts::claim::EvidenceCertificate;

    #[test]
    fn structural_witness_is_stable_and_order_sensitive() {
        let a = Evidence::TrustedKernel {
            provider: CapabilityProviderId(20),
            certificate: EvidenceCertificate::StructuralTermEquality {
                left: TermId(1),
                right: TermId(2),
            },
            summary: "ignored".into(),
        };
        let b = Evidence::TrustedKernel {
            provider: CapabilityProviderId(20),
            certificate: EvidenceCertificate::StructuralTermEquality {
                left: TermId(1),
                right: TermId(2),
            },
            summary: "different summary".into(),
        };
        let c = Evidence::TrustedKernel {
            provider: CapabilityProviderId(20),
            certificate: EvidenceCertificate::StructuralTermEquality {
                left: TermId(2),
                right: TermId(1),
            },
            summary: "ignored".into(),
        };
        assert_eq!(witness_ref_from_evidence(&a), witness_ref_from_evidence(&b));
        assert_ne!(witness_ref_from_evidence(&a), witness_ref_from_evidence(&c));
    }
}
