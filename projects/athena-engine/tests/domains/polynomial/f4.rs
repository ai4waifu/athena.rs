//! F4 Macaulay CSR 脚手架测试。

use athena_engine::domains::polynomial::{
    CoefficientDomain, MacaulayRowInput, MonomialOrder, PolynomialBuilder, RingTable, build_macaulay_csr,
};
use athena_numeric::Number;
use athena_types::SymbolId;

#[test]
fn macaulay_csr_two_rows_share_columns() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::small_int(1), vec![1]).unwrap(); // x
    b.push_term(Number::small_int(1), vec![0]).unwrap(); // + 1
    let f = b.build(&rings).unwrap();

    let zero = [0u32];
    let one = [1u32];
    let matrix = build_macaulay_csr(
        &[
            MacaulayRowInput { multiplier: &zero, polynomial: &f }, // x + 1
            MacaulayRowInput { multiplier: &one, polynomial: &f },  // x*(x+1) = x^2 + x
        ],
        &rings,
    )
    .unwrap();

    assert_eq!(matrix.nrows(), 2);
    // columns: x^2, x, 1 (Lex descending)
    assert_eq!(matrix.ncols(), 3);
    assert_eq!(matrix.columns[0], vec![2]);
    assert_eq!(matrix.columns[1], vec![1]);
    assert_eq!(matrix.columns[2], vec![0]);
    assert_eq!(matrix.nnz(), 4);
    // row0: x + 1 → cols 1 and 2
    assert_eq!(&matrix.col_idx[matrix.row_ptr[0]..matrix.row_ptr[1]], &[1, 2]);
    // row1: x^2 + x → cols 0 and 1
    assert_eq!(&matrix.col_idx[matrix.row_ptr[1]..matrix.row_ptr[2]], &[0, 1]);
}

#[test]
fn macaulay_rejects_empty_rows() {
    let rings = RingTable::new();
    let err = build_macaulay_csr(&[], &rings).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_DOMAIN_ERROR");
}
