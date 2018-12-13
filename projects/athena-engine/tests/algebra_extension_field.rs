//! 代数父对象 Phase 5：𝔽_{p^n} 多项式基 presentation 与元素运算。

use athena_engine::{
    FieldDescriptor, FieldElementRepr, FieldTable, Integer, PropertyState, add_field_elements, canonical_extension_element,
    inv_field_element, mul_field_elements,
};
use athena_types::DiagnosticCode;

fn gf4_modulus() -> Vec<Integer> {
    vec![Integer::one(), Integer::one(), Integer::one()]
}

fn gf8_modulus() -> Vec<Integer> {
    vec![Integer::one(), Integer::one(), Integer::zero(), Integer::one()]
}

#[test]
fn polynomial_basis_field_interns_idempotently() {
    let mut table = FieldTable::new();
    let f1 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let f2 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    assert_eq!(f1, f2);
    assert!(table.finite_field_poly_spec(f1).is_some());
}

#[test]
fn reducible_modulus_rejected() {
    let mut table = FieldTable::new();
    let err =
        table.polynomial_basis_field(Integer::from_i64(2), vec![Integer::zero(), Integer::zero(), Integer::one()]).unwrap_err();
    assert_eq!(err.code.as_str(), DiagnosticCode::FieldModulusReducible.as_str());
}

#[test]
fn gf4_mul_matches_known_arithmetic() {
    let mut table = FieldTable::new();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let one_plus_x = canonical_extension_element(&table, f4, vec![Integer::one(), Integer::one()]).unwrap();
    let sq = mul_field_elements(&table, &one_plus_x, &one_plus_x).unwrap();
    assert_eq!(sq.repr, FieldElementRepr::ExtensionCoords { coords: vec![Integer::zero(), Integer::one()] });
}

#[test]
fn gf4_inverse_of_x() {
    let mut table = FieldTable::new();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let x = canonical_extension_element(&table, f4, vec![Integer::zero(), Integer::one()]).unwrap();
    let inv = inv_field_element(&table, &x).unwrap();
    let prod = mul_field_elements(&table, &x, &inv).unwrap();
    assert_eq!(prod.repr, FieldElementRepr::ExtensionCoords { coords: vec![Integer::one(), Integer::zero()] });
}

#[test]
fn gf8_degree_three_extension() {
    let mut table = FieldTable::new();
    let f8 = table.polynomial_basis_field(Integer::from_i64(2), gf8_modulus()).unwrap();
    let spec = table.finite_field_poly_spec(f8).unwrap();
    assert_eq!(spec.degree, 3);
    match table.descriptor(f8).unwrap() {
        FieldDescriptor::Extension { degree, .. } => {
            assert!(matches!(degree, PropertyState::Proven { value: 3, .. }));
        }
        other => panic!("expected extension descriptor, got {other:?}"),
    }
}

#[test]
fn extension_field_has_same_characteristic_as_base() {
    let mut table = FieldTable::new();
    let f9 = table
        .polynomial_basis_field(Integer::from_i64(3), vec![Integer::from_i64(1), Integer::zero(), Integer::one()])
        .unwrap();
    assert_eq!(table.characteristic(f9), Some(Integer::from_i64(3)));
}

#[test]
fn extension_addition_componentwise() {
    let mut table = FieldTable::new();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let a = canonical_extension_element(&table, f4, vec![Integer::one(), Integer::zero()]).unwrap();
    let b = canonical_extension_element(&table, f4, vec![Integer::zero(), Integer::one()]).unwrap();
    let sum = add_field_elements(&table, &a, &b).unwrap();
    assert_eq!(sum.repr, FieldElementRepr::ExtensionCoords { coords: vec![Integer::one(), Integer::one()] });
}

#[test]
fn validate_finite_field_accepts_polynomial_basis() {
    let mut table = FieldTable::new();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    assert!(table.validate_finite_field(f4).is_ok());
}
