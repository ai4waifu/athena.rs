//! 自 `src/runtime/results/store.rs` 迁出的原内联测试。

use athena_engine::runtime::results::{ComputationResult, CoverageStatus, ResultProviderId, ResultProviderStamp};
use athena_types::ComputationStatus;

#[test]
fn provider_stamp_uses_contract_version() {
    let stamp = ResultProviderId::POLYNOMIAL.stamped();
    assert_eq!(stamp.id, ResultProviderId::POLYNOMIAL);
    assert_eq!(stamp.version, ResultProviderId::CONTRACT_VERSION);
    assert!(stamp.matches_current_contract());
}

#[test]
fn provider_stamp_rejects_stale_version() {
    let current = ResultProviderId::CALCULUS.stamped();
    let stale = ResultProviderStamp { id: ResultProviderId::CALCULUS, version: 0 };
    assert!(!current.compatible_with(stale));
    assert!(!stale.matches_current_contract());
}

#[test]
fn computation_result_with_provider_stamps_version() {
    let result = ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full).with_provider(ResultProviderId::NUMBER_THEORY);
    assert_eq!(result.provider, Some(ResultProviderId::NUMBER_THEORY.stamped()));
    assert_eq!(result.coverage, CoverageStatus::Full);
}
