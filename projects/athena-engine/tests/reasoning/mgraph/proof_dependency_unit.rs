//! 自 `src/reasoning/mgraph/facts/proof_dependency.rs` 迁出的原内联测试。

use athena_engine::reasoning::mgraph::facts::{FactId, ProofDependencyIndex};

#[test]
fn records_and_queries_transitive_dependency() {
    let mut index = ProofDependencyIndex::new();
    index.record(FactId(1), &[]).unwrap();
    index.record(FactId(2), &[FactId(1)]).unwrap();
    index.record(FactId(3), &[FactId(2)]).unwrap();
    assert!(index.depends_on(FactId(3), FactId(1)));
    assert!(index.depends_on(FactId(3), FactId(2)));
    assert!(!index.depends_on(FactId(1), FactId(3)));
    assert_eq!(index.premises(FactId(3)), &[FactId(2)]);
}

#[test]
fn rejects_self_and_future_premises() {
    let mut index = ProofDependencyIndex::new();
    let err = index.record(FactId(1), &[FactId(1)]).expect_err("self");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("self_dependency"));
    let err = index.record(FactId(1), &[FactId(2)]).expect_err("future");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("premise_not_prior"));
}
