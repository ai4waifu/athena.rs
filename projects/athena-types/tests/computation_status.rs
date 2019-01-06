//! SEM2 `ComputationStatus` 合同。

use athena_types::ComputationStatus;

#[test]
fn computation_status_rejects_bool_collapse() {
    assert!(ComputationStatus::Exact.is_unconditional_exact());
    assert!(ComputationStatus::Verified.is_unconditional_exact());
    assert!(!ComputationStatus::Partial.is_unconditional_exact());
    assert!(!ComputationStatus::Probable.is_unconditional_exact());
    assert!(ComputationStatus::ResourceLimited.is_resource_limited());
    assert!(ComputationStatus::Candidate.must_surface());
    assert!(!ComputationStatus::Exact.must_surface());
    assert_eq!(ComputationStatus::Invalid.name(), "Invalid");
}
