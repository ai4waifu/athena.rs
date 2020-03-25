//! 自 `src/reasoning/mgraph/equivalence/proof_forest.rs` 迁出的原内联测试。

use athena_types::TermId;

use athena_engine::{Session, reasoning::mgraph::equivalence::*};

#[test]
fn proof_forest_records_admitted_equality_edges() {
    let mut forest = ProofForest::new();
    forest.record(TermId(1), TermId(2), ProofStepKind::AdmittedEquality);
    assert_eq!(forest.len(), 1);
    assert_eq!(forest.edges()[0].step_kind, ProofStepKind::AdmittedEquality);
}
