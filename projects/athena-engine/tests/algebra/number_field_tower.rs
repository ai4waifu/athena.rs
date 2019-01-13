//! `$\mathbb{Q}(\alpha)$` 数域幂基与相对塔。

use athena_engine::{
    FieldDescriptor, FieldElementRepr, FieldTable, Integer, PropertyState, Rational, canonical_number_field_element,
    inv_field_element, mul_field_elements,
};
use athena_types::DiagnosticCode;

fn q_poly_x2_minus(n: i64) -> Vec<Rational> {
    vec![Rational::from_integer(Integer::from_i64(-n)), Rational::zero(), Rational::one()]
}

#[test]
fn q_sqrt2_from_x2_minus_2_interns_idempotently() {
    let mut table = FieldTable::new();
    let k1 = table.number_field_from_minimal_polynomial(q_poly_x2_minus(2)).unwrap();
    let k2 = table.number_field_from_minimal_polynomial(q_poly_x2_minus(2)).unwrap();
    assert_eq!(k1, k2);
    assert!(table.number_field_spec(k1).is_some());
}

#[test]
fn reducible_x2_minus_1_rejected() {
    let mut table = FieldTable::new();
    let err = table.number_field_from_minimal_polynomial(q_poly_x2_minus(1)).unwrap_err();
    assert_eq!(err.code.as_str(), DiagnosticCode::FieldModulusReducible.as_str());
}

#[test]
fn sqrt2_squared_is_2() {
    let mut table = FieldTable::new();
    let k = table.number_field_from_minimal_polynomial(q_poly_x2_minus(2)).unwrap();
    let sqrt2 = canonical_number_field_element(&table, k, vec![Rational::zero(), Rational::one()]).unwrap();
    let sq = mul_field_elements(&table, &sqrt2, &sqrt2).unwrap();
    assert_eq!(
        sq.repr,
        FieldElementRepr::NumberFieldCoords { coords: vec![Rational::from_integer(Integer::from_i64(2)), Rational::zero()] }
    );
}

#[test]
fn inv_of_sqrt2() {
    let mut table = FieldTable::new();
    let k = table.number_field_from_minimal_polynomial(q_poly_x2_minus(2)).unwrap();
    let sqrt2 = canonical_number_field_element(&table, k, vec![Rational::zero(), Rational::one()]).unwrap();
    let inv = inv_field_element(&table, &sqrt2).unwrap();
    let prod = mul_field_elements(&table, &sqrt2, &inv).unwrap();
    assert_eq!(prod.repr, FieldElementRepr::NumberFieldCoords { coords: vec![Rational::one(), Rational::zero()] });
}

#[test]
fn minpoly_of_sqrt2_is_x2_minus_2() {
    let mut table = FieldTable::new();
    let k = table.number_field_from_minimal_polynomial(q_poly_x2_minus(2)).unwrap();
    let minpoly = table.minimal_polynomial_over_rationals(k, &[Rational::zero(), Rational::one()]).unwrap();
    assert_eq!(minpoly, q_poly_x2_minus(2));
}

#[test]
fn relative_tower_q_sqrt2_sqrt3() {
    let mut table = FieldTable::new();
    let q = table.rationals();
    let k2 = table.number_field_from_minimal_polynomial(q_poly_x2_minus(2)).unwrap();
    let k23 = table.relative_number_field(k2, q_poly_x2_minus(3)).unwrap();
    let ext = table.extension_by_field(k23).expect("extension");
    assert_eq!(ext.base, k2);
    assert_eq!(ext.proven_degree(), Some(2));
    let spec = table.number_field_spec(k23).unwrap();
    assert_eq!(spec.absolute_degree, 4);
    assert_eq!(spec.relative_degree, 2);
    let tower = table.extension_tower(ext.id).expect("tower");
    assert_eq!(tower, vec![q, k2, k23]);
    match table.descriptor(k23).unwrap() {
        FieldDescriptor::Extension { degree, .. } => {
            assert!(matches!(degree, PropertyState::Proven { value: 2, .. }));
        }
        other => panic!("expected extension descriptor, got {other:?}"),
    }
}

#[test]
fn sqrt3_squared_is_3_in_tower() {
    let mut table = FieldTable::new();
    let k2 = table.number_field_from_minimal_polynomial(q_poly_x2_minus(2)).unwrap();
    let k23 = table.relative_number_field(k2, q_poly_x2_minus(3)).unwrap();
    let sqrt3 = canonical_number_field_element(
        &table,
        k23,
        vec![Rational::zero(), Rational::zero(), Rational::one(), Rational::zero()],
    )
    .unwrap();
    let sq = mul_field_elements(&table, &sqrt3, &sqrt3).unwrap();
    assert_eq!(
        sq.repr,
        FieldElementRepr::NumberFieldCoords {
            coords: vec![Rational::from_integer(Integer::from_i64(3)), Rational::zero(), Rational::zero(), Rational::zero()]
        }
    );
}

#[test]
fn adjoining_sqrt2_again_over_q_sqrt2_rejected() {
    let mut table = FieldTable::new();
    let k2 = table.number_field_from_minimal_polynomial(q_poly_x2_minus(2)).unwrap();
    let err = table.relative_number_field(k2, q_poly_x2_minus(2)).unwrap_err();
    assert_eq!(err.code.as_str(), DiagnosticCode::FieldExtensionInvalid.as_str());
}
