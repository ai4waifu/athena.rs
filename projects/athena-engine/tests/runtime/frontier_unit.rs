//! 自 `src/runtime/frontier.rs` 迁出的原内联测试。

use athena_engine::{
    domains::solve::{ResumeKind, ResumeToken},
    runtime::{
        frontier::{ComputationFrontier, FrontierStore, ResumeCheck},
        results::{ResultProviderId, ResultProviderStamp},
    },
};
use athena_types::{AssumptionSetId, Diagnostic, DiagnosticCode, FrontierId};
use std::collections::BTreeMap;

fn check<'a>(
    provider: athena_engine::runtime::results::ResultProviderStamp,
    assumption_scope: Option<AssumptionSetId>,
    goal: u64,
    plan: Option<u64>,
    objects: &'a [u64],
    certs: &'a [u64],
    cancelled: bool,
    budget_limit: Option<u64>,
) -> ResumeCheck<'a> {
    ResumeCheck {
        provider,
        assumption_scope,
        goal_fingerprint: goal,
        plan_fingerprint: plan,
        object_fingerprints: objects,
        available_certificates: certs,
        cancelled,
        budget_limit,
    }
}

#[test]
fn insert_and_get_frontier() {
    let stamp = ResultProviderId::POLYNOMIAL.stamped();
    let resume = ResumeToken::empty_with_provider(ResumeKind::UnivariateFactor, stamp);
    let mut frontier = ComputationFrontier::new(0xA11CE, resume);
    frontier.plan_fingerprint = Some(0xBEEF);
    frontier.object_fingerprints = vec![1, 2, 3];
    frontier.assumption_scope = Some(AssumptionSetId(9));
    frontier.budget_consumed = 4;

    let mut store = FrontierStore::new();
    let id = store.insert(frontier.owning_copy());
    assert!(store.contains(id));
    assert_eq!(store.get(id), Some(&frontier));
    assert_eq!(store.count(), 1);
}

#[test]
fn resume_gate_rejects_stale_provider() {
    let stamp = ResultProviderId::LINEAR_ALGEBRA.stamped();
    let frontier = ComputationFrontier::new(1, ResumeToken::empty_with_provider(ResumeKind::LinearExact, stamp));
    assert!(frontier.resume_provider_gate(stamp).is_ok());
    let stale = athena_engine::runtime::results::ResultProviderStamp { id: ResultProviderId::LINEAR_ALGEBRA, version: 0 };
    let err = frontier.resume_provider_gate(stale).expect_err("stale");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("provider_version_incompatible"));
}

#[test]
fn validate_resume_checks_assumption_and_fingerprints() {
    let stamp = ResultProviderId::POLYNOMIAL.stamped();
    let mut frontier = ComputationFrontier::new(0x11, ResumeToken::empty_with_provider(ResumeKind::Cut, stamp));
    frontier.plan_fingerprint = Some(0x22);
    frontier.object_fingerprints = vec![7, 8];
    frontier.assumption_scope = Some(AssumptionSetId(3));

    assert!(frontier.validate_resume(check(stamp, Some(AssumptionSetId(3)), 0x11, Some(0x22), &[7, 8], &[], false, None)).is_ok());

    let scope_err =
        frontier.validate_resume(check(stamp, Some(AssumptionSetId(4)), 0x11, Some(0x22), &[7, 8], &[], false, None)).expect_err("scope");
    assert_eq!(scope_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("assumption_scope_changed"));

    let goal_err =
        frontier.validate_resume(check(stamp, Some(AssumptionSetId(3)), 0x99, Some(0x22), &[7, 8], &[], false, None)).expect_err("goal");
    assert_eq!(goal_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("goal_fingerprint_mismatch"));

    let objects_err =
        frontier.validate_resume(check(stamp, Some(AssumptionSetId(3)), 0x11, Some(0x22), &[7], &[], false, None)).expect_err("objects");
    assert_eq!(objects_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("object_fingerprints_mismatch"));
}

#[test]
fn validate_resume_rejects_missing_certificate_and_budget() {
    let stamp = ResultProviderId::NUMBER_THEORY.stamped();
    let mut frontier = ComputationFrontier::new(1, ResumeToken::empty_with_provider(ResumeKind::Cut, stamp));
    frontier.certificate_fingerprints = vec![100, 200];
    frontier.budget_consumed = 5;

    assert!(frontier.validate_resume(check(stamp, None, 1, None, &[], &[200, 100, 300], false, Some(10))).is_ok());

    let cert_err = frontier.validate_resume(check(stamp, None, 1, None, &[], &[100], false, Some(10))).expect_err("cert");
    assert_eq!(cert_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("certificate_not_replayable"));

    let cancel_err = frontier.validate_resume(check(stamp, None, 1, None, &[], &[100, 200], true, Some(10))).expect_err("cancel");
    assert_eq!(cancel_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("cancelled"));

    let budget_err = frontier.validate_resume(check(stamp, None, 1, None, &[], &[100, 200], false, Some(5))).expect_err("budget");
    assert_eq!(budget_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("budget_exhausted"));
}
