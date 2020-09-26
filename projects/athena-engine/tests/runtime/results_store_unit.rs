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

#[test]
fn result_store_links_derived_from_parent() {
    use athena_engine::runtime::results::ResultStore;
    use athena_types::{Condition, Predicate, ResultId, TermId};

    let mut store = ResultStore::new();
    let parent_cond = Condition { predicate: Predicate::NonZero(TermId(1)), resolved: false };
    let parent = store.insert(
        ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full).with_condition(parent_cond.clone()),
    );
    let child = store.insert(ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full));
    assert!(store.link_derived_from(child, parent));
    assert_eq!(store.get(child).and_then(|r| r.derived_from), Some(parent));
    assert_eq!(store.get(child).map(|r| r.conditions.as_slice()), Some([parent_cond].as_slice()));
    assert!(!store.link_derived_from(child, ResultId(999)));
}
