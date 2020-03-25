//! 自 `src/domains/views/polynomial_matrix.rs` 迁出的原内联测试。

use athena_engine::domains::{
    polynomial::{Polynomial, PolynomialObjectStore, RingTable},
    views::{PolynomialMatrixView, ViewKind},
};
use athena_types::RingId;

#[test]
fn polynomial_matrix_view_borrows_zero_poly() {
    let rings = RingTable::default();
    let mut store = PolynomialObjectStore::new();
    let r = store.intern(Polynomial::zero(RingId(0)), &rings);
    let view = PolynomialMatrixView::open(&store, r).expect("view");
    assert_eq!(view.header().kind, ViewKind::PolynomialMatrix);
    assert!(view.is_zero());
    assert_eq!(view.nnz(), 0);
    assert_eq!(view.ring(), RingId(0));
}
