//! 有限域元素骨架。

use athena_numeric::{FiniteFieldValue, Integer, NumericValue};
use athena_types::{FieldId, FieldPresentationId};

#[test]
fn zero_element_is_single_zero_coeff() {
    let v = FiniteFieldValue::zero(FieldId(3), FieldPresentationId(1));
    v.validate().unwrap();
    assert_eq!(v.coefficients().len(), 1);
    assert!(v.coefficients()[0].is_zero());
    assert_eq!(v.presentation(), FieldPresentationId(1));
    assert!(NumericValue::finite_field(v).validate().is_ok());
}

#[test]
fn rejects_empty_coefficients() {
    let err = FiniteFieldValue::try_new(FieldId(1), FieldPresentationId(0), vec![]).unwrap_err();
    assert_eq!(
        err.details.get("operation").map(|d| d.to_string()).as_deref(),
        Some("finite_field_empty_coefficients")
    );
}

#[test]
fn accepts_reduced_coordinates() {
    let v =
        FiniteFieldValue::try_new(FieldId(2), FieldPresentationId(9), vec![Integer::from_i64(1), Integer::from_i64(-1)])
            .unwrap();
    assert_eq!(v.field(), FieldId(2));
    assert_eq!(v.presentation(), FieldPresentationId(9));
    assert!(matches!(v.repr(), athena_numeric::FiniteFieldRepr::Coefficients(_)));
    NumericValue::finite_field(v).validate().unwrap();
}

#[test]
fn try_from_repr_rejects_empty_coefficients_variant() {
    use athena_numeric::FiniteFieldRepr;
    let err = FiniteFieldValue::try_from_repr(
        FieldId(0),
        FieldPresentationId(0),
        FiniteFieldRepr::Coefficients(vec![]),
    )
    .unwrap_err();
    assert_eq!(
        err.details.get("operation").map(|d| d.to_string()).as_deref(),
        Some("finite_field_empty_coefficients")
    );
}
