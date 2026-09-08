//! 代数父对象核心合同测试。

use athena_engine::domains::algebra::{AlgebraParentId, CoefficientParent, FieldTable};
use athena_numeric::{FiniteFieldValue, Integer};
use athena_types::FieldId;

#[test]
fn field_table_prime_field_interns() {
    let mut table = FieldTable::new();
    let q = table.rationals();
    let f5a = table.prime_field(Integer::from_i64(5)).unwrap();
    let f5b = table.prime_field(Integer::from_i64(5)).unwrap();
    assert_eq!(f5a, f5b);
    assert_ne!(q, f5a);
    assert!(table.presentation(f5a).is_some());
}

#[test]
fn coefficient_parent_roundtrip() {
    let parent = CoefficientParent::Field(FieldId(2));
    assert_eq!(parent.as_algebra_parent(), Some(AlgebraParentId::Field(FieldId(2))));
}

#[test]
fn finite_field_value_stores_coefficients_not_term_handles() {
    let v = FiniteFieldValue::try_new(FieldId(0), athena_types::FieldPresentationId(0), vec![Integer::from_i64(1)]).unwrap();
    assert_eq!(v.field(), FieldId(0));
    assert_eq!(v.coefficients(), &[Integer::from_i64(1)]);
}
