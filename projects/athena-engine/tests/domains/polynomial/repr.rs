//! 多项式表示族：DenseUnivariate / SparseUnivariate / DistributedSparse 往返与数学相等。

use athena_engine::domains::polynomial::{
    CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialRepr, PolynomialReprBody, ReprTarget, RingTable, reprs_mathematically_equal,
};
use athena_numeric::Number;
use athena_types::SymbolId;

fn univariate_x_ring() -> (RingTable, athena_engine::domains::polynomial::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern(CoefficientDomain::Integer, vec![SymbolId(1)], MonomialOrder::Lex).expect("valid ring");
    (rings, id)
}

fn sample_univariate(rings: &RingTable, ring: athena_engine::domains::polynomial::RingId) -> athena_engine::domains::polynomial::Polynomial {
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(3), vec![0]).unwrap();
    b.push_term(Number::small_int(2), vec![1]).unwrap();
    b.push_term(Number::small_int(1), vec![2]).unwrap();
    b.build(rings).unwrap()
}

#[test]
fn distributed_sparse_roundtrip() {
    let (rings, ring) = univariate_x_ring();
    let poly = sample_univariate(&rings, ring);
    let repr = PolynomialRepr::from_polynomial(&poly, &rings, ReprTarget::DistributedSparse).unwrap();
    assert!(matches!(repr.body, PolynomialReprBody::DistributedSparse { .. }));
    let back = repr.to_polynomial(&rings).unwrap();
    assert_eq!(poly, back);
}

#[test]
fn dense_univariate_roundtrip() {
    let (rings, ring) = univariate_x_ring();
    let poly = sample_univariate(&rings, ring);
    let repr = PolynomialRepr::from_polynomial(&poly, &rings, ReprTarget::DenseUnivariate { var_index: 0 }).unwrap();
    match &repr.body {
        PolynomialReprBody::DenseUnivariate { coefficients, .. } => {
            assert_eq!(coefficients.len(), 3);
            assert_eq!(coefficients[0].to_render_string(), "3");
            assert_eq!(coefficients[2].to_render_string(), "1");
        }
        _ => panic!("expected dense univariate"),
    }
    let back = repr.to_polynomial(&rings).unwrap();
    assert_eq!(poly, back);
}

#[test]
fn sparse_univariate_roundtrip() {
    let (rings, ring) = univariate_x_ring();
    let poly = sample_univariate(&rings, ring);
    let repr = PolynomialRepr::from_polynomial(&poly, &rings, ReprTarget::SparseUnivariate { var_index: 0 }).unwrap();
    match &repr.body {
        PolynomialReprBody::SparseUnivariate { terms, .. } => {
            assert_eq!(terms.len(), 3);
            assert_eq!(terms[0].0, 2);
        }
        _ => panic!("expected sparse univariate"),
    }
    let back = repr.to_polynomial(&rings).unwrap();
    assert_eq!(poly, back);
}

#[test]
fn repr_conversion_preserves_equality() {
    let (rings, ring) = univariate_x_ring();
    let poly = sample_univariate(&rings, ring);
    let dense = PolynomialRepr::from_polynomial(&poly, &rings, ReprTarget::DenseUnivariate { var_index: 0 }).unwrap();
    let sparse = PolynomialRepr::from_polynomial(&poly, &rings, ReprTarget::SparseUnivariate { var_index: 0 }).unwrap();
    let dist = PolynomialRepr::from_polynomial(&poly, &rings, ReprTarget::DistributedSparse).unwrap();
    assert!(reprs_mathematically_equal(&dense, &sparse, &rings).unwrap());
    assert!(reprs_mathematically_equal(&dense, &dist, &rings).unwrap());
    assert!(reprs_mathematically_equal(&sparse, &dist, &rings).unwrap());
}

#[test]
fn convert_dense_to_sparse_in_ring() {
    let (rings, ring) = univariate_x_ring();
    let poly = sample_univariate(&rings, ring);
    let dense = PolynomialRepr::from_polynomial(&poly, &rings, ReprTarget::DenseUnivariate { var_index: 0 }).unwrap();
    let sparse = dense.convert(&rings, ReprTarget::SparseUnivariate { var_index: 0 }).unwrap();
    assert!(matches!(sparse.body, PolynomialReprBody::SparseUnivariate { .. }));
    assert_eq!(sparse.to_polynomial(&rings).unwrap(), poly);
}

#[test]
fn multivariate_rejects_univariate_dense() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Integer, vec![SymbolId(1), SymbolId(2)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![1, 1]).unwrap();
    let poly = b.build(&rings).unwrap();
    let err = PolynomialRepr::from_polynomial(&poly, &rings, ReprTarget::DenseUnivariate { var_index: 0 }).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_POLYNOMIAL_VARIABLE_MISMATCH");
}

#[test]
fn zero_polynomial_dense_is_empty() {
    let (rings, ring) = univariate_x_ring();
    let poly = athena_engine::domains::polynomial::Polynomial::zero(ring);
    let repr = PolynomialRepr::from_polynomial(&poly, &rings, ReprTarget::DenseUnivariate { var_index: 0 }).unwrap();
    match repr.body {
        PolynomialReprBody::DenseUnivariate { ref coefficients, .. } => assert!(coefficients.is_empty()),
        _ => panic!("expected dense"),
    }
    assert_eq!(repr.to_polynomial(&rings).unwrap(), poly);
}
