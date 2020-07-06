//! 自 `src/reasoning/mgraph/relations/derived.rs` 迁出的原内联测试。
//!
//! 不得经 `VerifiedClaim::from_admission` / `AdmissionJournal::append` 伪造（已 crate 密封）。
//! 派生索引重建经真实 `AdmissionGate` 写入后再 `rebuild_*`。

use athena_types::TermId;

use athena_engine::reasoning::mgraph::{
    AdmissionGate, CapabilityProviderId, Claim, Evidence, EvidenceCertificate, Guarantee, ProofStepKind, Proposition, Scope, SemanticCore,
    VerificationPolicy, relations::*,
};
use athena_ir::TermStore;

fn admit_term_eq_harness(store: &mut TermStore, semantic: &mut SemanticCore, left: u32, right: u32) {
    AdmissionGate::admit_claim(
        store,
        semantic,
        Claim {
            proposition: Proposition::TermEquality { left: TermId(left), right: TermId(right) },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CapabilityProviderId(0),
                certificate: EvidenceCertificate::TestHarness,
                summary: String::new(),
            },
        },
        &VerificationPolicy::for_test_harness(), None)
    .expect("admit term eq harness");
}

fn admit_congruence(store: &mut TermStore, semantic: &mut SemanticCore, modulus: u64, left: u64, right: u64) {
    AdmissionGate::admit_congruence(store, semantic, modulus, left, right, &VerificationPolicy::default()).expect("admit congruence");
}

#[test]
fn rebuild_projects_term_equality_into_uf_and_proof_forest() {
    let mut store = TermStore::new();
    let mut semantic = SemanticCore::new();
    admit_term_eq_harness(&mut store, &mut semantic, 1, 2);
    admit_term_eq_harness(&mut store, &mut semantic, 2, 3);
    semantic.rebuild_derived();
    let derived = &semantic.derived;
    assert_eq!(derived.exact_uf.find(TermId(1)), derived.exact_uf.find(TermId(3)));
    assert_eq!(derived.proof_forest.len(), 2);
    assert_eq!(derived.proof_forest.edges()[0].step_kind, ProofStepKind::AdmittedEquality);
}

#[test]
fn rebuild_projects_congruence_into_fingerprint_index() {
    let mut store = TermStore::new();
    let mut semantic = SemanticCore::new();
    admit_congruence(&mut store, &mut semantic, 97, 10, 20);
    admit_congruence(&mut store, &mut semantic, 97, 20, 30);
    semantic.rebuild_derived();
    let derived = &semantic.derived;
    assert_eq!(derived.congruence.find(97, 10), derived.congruence.find(97, 30));
    assert_eq!(derived.congruence.union_count(), 2);
    assert!(derived.proof_forest.is_empty());
    assert_eq!(derived.exact_uf.union_count(), 0);
}

#[test]
fn rebuild_keeps_congruence_classes_per_modulus() {
    let mut store = TermStore::new();
    let mut semantic = SemanticCore::new();
    admit_congruence(&mut store, &mut semantic, 7, 10, 20);
    admit_congruence(&mut store, &mut semantic, 11, 10, 30);
    semantic.rebuild_derived();
    let derived = &semantic.derived;
    assert_eq!(derived.congruence.find(7, 10), derived.congruence.find(7, 20));
    assert_ne!(derived.congruence.find(7, 10), derived.congruence.find(7, 30));
    assert_eq!(derived.congruence.modulus_count(), 2);
}

#[test]
fn proof_forest_step_kind_follows_term_equality_certificate() {
    let mut store = TermStore::new();
    let mut semantic = SemanticCore::new();
    // ApplicationCongruence / TypedRewriteReplay 需专用上下文；此处用 harness 写入后，
    // 再经 journal 重建验证 step_kind 映射——改由直接检查 DerivedIndexes::apply 路径：
    // 先 admit harness（AdmittedEquality），再用二次 rebuild 保持计数。
    admit_term_eq_harness(&mut store, &mut semantic, 1, 2);
    assert_eq!(semantic.derived.proof_forest.edges()[0].step_kind, ProofStepKind::AdmittedEquality);

    // Congruence / rewrite 种类映射仍由 crate 内 `proof_step_from_evidence` 覆盖；
    // 集成面只保证 harness 种子可重建。
    semantic.rebuild_from_journal();
    assert_eq!(semantic.derived.proof_forest.len(), 1);
    assert_eq!(semantic.derived.proof_forest.edges()[0].step_kind, ProofStepKind::AdmittedEquality);
}
