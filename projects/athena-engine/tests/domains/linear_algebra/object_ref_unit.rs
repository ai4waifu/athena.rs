//! 自 `src/domains/linear_algebra/object_ref.rs` 迁出的原内联测试。

use athena_engine::domains::linear_algebra::{MatrixObjectStore, MatrixValue};
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
    assert!(store.object_ref(r0).is_some());
}
