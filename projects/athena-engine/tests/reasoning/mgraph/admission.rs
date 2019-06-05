//! Semantic core 合同：FactLog 单调性 · 派生索引可重建。

use athena_engine::reasoning::mgraph::{
    AdmissionGate, Claim, Evidence, FactId, Guarantee, MGraphState, POLYNOMIAL_PROVIDER_ID, Proposition, Scope, SemanticCore,
    VerificationPolicy,
};

#[test]
fn fact_log_is_append_only_monotonic() {
    let mut core = SemanticCore::new();
    assert_eq!(core.fact_log.count(), 0);
    let id0 = admit_ok(&mut core, sample_claim(Guarantee::ProvenExact, 1));
    let id1 = admit_ok(&mut core, sample_claim(Guarantee::ProvenExact, 2));
    assert_eq!(id0, FactId(0));
    assert_eq!(id1, FactId(1));
    assert_eq!(core.fact_log.count(), 2);
    assert!(core.fact_log.get(FactId(0)).is_some());
}

#[test]
fn derived_indexes_rebuild_matches_incremental() {
    let mut core = SemanticCore::new();
    admit_ok(&mut core, sample_claim(Guarantee::ProvenExact, 10));
    admit_ok(&mut core, sample_claim(Guarantee::ProvenExact, 20));
    let incremental_witnesses = core.derived.rewrite_witnesses.len();
    core.rebuild_derived();
    assert_eq!(core.derived.rewrite_witnesses.len(), incremental_witnesses);
}

#[test]
fn mgraph_state_splits_semantic_and_operational() {
    let state = MGraphState::new();
    assert!(state.semantic.fact_log.is_empty());
    assert!(state.operational.result_cache.polynomial.is_empty());
    assert!(state.operational.hyper_edges.is_empty());
}

fn admit_ok(semantic: &mut SemanticCore, claim: Claim) -> FactId {
    AdmissionGate::admit_claim(semantic, claim, &VerificationPolicy::default()).expect("should admit")
}

fn sample_claim(guarantee: Guarantee, fingerprint: u64) -> Claim {
    Claim {
        proposition: Proposition::PolynomialResult {
            operation: athena_engine::domains::polynomial::PolynomialCacheOp::Add,
            request_fingerprint: fingerprint,
        },
        scope: Scope::Unconditional,
        guarantee,
        evidence: Evidence::TrustedKernel { provider: POLYNOMIAL_PROVIDER_ID, summary: "test".into() },
    }
}
