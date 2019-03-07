//! 有限域元素骨架。

use athena_numeric::{FiniteFieldValue, Integer, NumericValue};
use athena_types::FieldId;

#[test]
fn zero_element_is_single_zero_coeff() {
    let v = FiniteFieldValue::zero(FieldId(3));
    v.validate().unwrap();
    assert_eq!(v.coefficients.len(), 1);
    assert!(v.coefficients[0].is_zero());
    assert!(NumericValue::finite_field(v).validate().is_ok());
}

#[test]
fn rejects_empty_coefficients() {
    let err = FiniteFieldValue::try_new(FieldId(1), vec![]).unwrap_err();
    assert_eq!(err.details.get("operation").map(|d| d.to_string()).as_deref(), Some("finite_field_empty_coefficients"));
}

#[test]
fn accepts_reduced_coordinates() {
    let v = FiniteFieldValue::try_new(FieldId(2), vec![Integer::from_i64(1), Integer::from_i64(-1)]).unwrap();
    assert_eq!(v.field, FieldId(2));
    NumericValue::finite_field(v).validate().unwrap();
}
