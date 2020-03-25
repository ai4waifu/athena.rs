//! 自 `src/domains/views/series_polynomial.rs` 迁出的原内联测试。

use athena_engine::domains::{
    calculus::{Remainder, Series, SeriesObjectStore},
    views::{SeriesPolynomialView, ViewKind},
};
use athena_types::{SymbolId, TermId};

#[test]
fn series_polynomial_view_borrows_terms_without_copy() {
    let mut store = SeriesObjectStore::new();
    let series = Series {
        variable: SymbolId(0),
        center: TermId(0),
        terms: vec![(TermId(1), 0), (TermId(2), 1)],
        order: 1,
        remainder: Remainder::ExactTruncation,
    };
    let r = store.intern(series);
    let view = SeriesPolynomialView::open(&store, r).expect("view");
    assert_eq!(view.header().kind, ViewKind::SeriesPolynomial);
    assert_eq!(view.term_count(), 2);
    assert_eq!(view.terms()[1], (TermId(2), 1));
    assert_eq!(view.variable(), SymbolId(0));
}
