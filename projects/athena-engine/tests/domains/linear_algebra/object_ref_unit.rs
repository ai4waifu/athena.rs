//! 自 `src/domains/linear_algebra/object_ref.rs` 迁出的原内联测试。

use athena_engine::domains::linear_algebra::{
    AlgorithmGuarantee, ElementParentKind, MatrixObjectStore, MatrixResult, MatrixValue,
};
use athena_numeric::Integer;

#[test]
fn intern_dedupes_identical_matrices() {
    let mut store = MatrixObjectStore::new();
    let a = MatrixValue::from_integers_row_major(1, 2, vec![Integer::from_i64(1), Integer::from_i64(2)]).unwrap();
    let b = MatrixValue::from_integers_row_major(1, 2, vec![Integer::from_i64(1), Integer::from_i64(2)]).unwrap();
    let r0 = store.intern(a);
    let r1 = store.intern(b);
    assert_eq!(r0, r1);
    assert_eq!(store.len(), 1);
    assert_eq!(store.revision(r0), Some(0));
    assert!(store.object_ref(r0).is_some());
}

#[test]
fn matrix_result_from_owned_snapshots_shape_and_domain() {
    let value = MatrixValue::from_integers_row_major(2, 1, vec![Integer::from_i64(3), Integer::from_i64(4)]).unwrap();
    let shape = value.shape();
    let outcome = MatrixResult::from_owned(value, AlgorithmGuarantee::Exact);
    assert_eq!(outcome.shape, shape);
    assert_eq!(outcome.element_domain, ElementParentKind::Integers);
    assert_eq!(outcome.guarantee, AlgorithmGuarantee::Exact);
    assert!(outcome.matrix_ref.is_none());
    assert!(outcome.revision.is_none());
    assert!(outcome.residual_inf.is_none());
}

#[test]
fn matrix_result_with_ref_carries_revision() {
    let mut store = MatrixObjectStore::new();
    let value = MatrixValue::from_integers_row_major(1, 1, vec![Integer::from_i64(9)]).unwrap();
    let matrix_ref = store.intern(value.owning_copy());
    let outcome = MatrixResult::from_owned(value, AlgorithmGuarantee::Exact).with_ref(matrix_ref, store.revision(matrix_ref).unwrap());
    assert_eq!(outcome.matrix_ref, Some(matrix_ref));
    assert_eq!(outcome.revision, Some(0));
}
