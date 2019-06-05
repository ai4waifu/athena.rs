//! 代数数骨架不变量。

use athena_numeric::{AlgebraicNumber, AlgebraicRepresentation, Interval, NumericContext, PolynomialFingerprint, Real};

#[test]
fn placeholder_requires_zero_fingerprint() {
    let iv = Interval::try_point(Real::machine(2.0)).unwrap();
    AlgebraicNumber::placeholder(iv.try_clone_in(&NumericContext::portable_default()).unwrap()).unwrap();
    let err = AlgebraicNumber::try_new(PolynomialFingerprint(1), iv, AlgebraicRepresentation::Placeholder).unwrap_err();
    assert_eq!(err.details.get("operation").map(|v| v.to_string()).as_deref(), Some("algebraic_placeholder_fingerprint"));
}

#[test]
fn minpoly_fingerprint_must_match() {
    let iv = Interval::try_point(Real::machine(0.0)).unwrap();
    let err = AlgebraicNumber::try_new(
        PolynomialFingerprint(1),
        iv,
        AlgebraicRepresentation::MinimalPolynomial { polynomial: PolynomialFingerprint(2), root_index: 0 },
    )
    .unwrap_err();
    assert_eq!(err.details.get("operation").map(|v| v.to_string()).as_deref(), Some("algebraic_fingerprint_mismatch"));
}

#[test]
fn rejects_empty_isolating_interval() {
    let err = AlgebraicNumber::try_new(
        PolynomialFingerprint(3),
        Interval::empty(),
        AlgebraicRepresentation::MinimalPolynomial { polynomial: PolynomialFingerprint(3), root_index: 1 },
    )
    .unwrap_err();
    assert_eq!(err.details.get("operation").map(|v| v.to_string()).as_deref(), Some("algebraic_empty_interval"));
}

#[test]
fn minpoly_round_constructs() {
    let iv = Interval::try_bounded(Real::machine(1.4), Real::machine(1.5), athena_numeric::IntervalDecoration::Certain).unwrap();
    let a = AlgebraicNumber::try_new(
        PolynomialFingerprint(7),
        iv,
        AlgebraicRepresentation::MinimalPolynomial { polynomial: PolynomialFingerprint(7), root_index: 0 },
    )
    .unwrap();
    a.validate().unwrap();
    assert!(athena_numeric::NumericValue::algebraic(a).validate().is_ok());
}
