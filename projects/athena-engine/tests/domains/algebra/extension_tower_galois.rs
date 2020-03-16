//! 域扩张塔与 Galois 群（`𝔽_{pⁿ}/𝔽_p`）。

use athena_engine::domains::{
    algebra::{FieldExtension, FieldTable, GroupTable, field_automorphism, frobenius_coords, is_galois_extension},
    field::{apply_field_automorphism, apply_prime_subfield_embedding, canonical_extension_element, canonical_prime_residue},
    galois::{GaloisComputation, GaloisDomainValue, GaloisRequest, GaloisResult, execute_galois_with_tables},
};
use athena_numeric::Integer;

fn gf4_modulus() -> Vec<Integer> {
    vec![Integer::one(), Integer::one(), Integer::one()]
}

#[test]
fn polynomial_basis_registers_field_extension_and_tower() {
    let mut table = FieldTable::new();
    let f2 = table.prime_field(Integer::from_i64(2)).unwrap();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let ext = table.extension_by_field(f4).expect("extension record");
    assert_eq!(ext.base, f2);
    assert_eq!(ext.proven_degree(), Some(2));
    let tower = table.extension_tower(ext.id).expect("tower");
    assert_eq!(tower, vec![f2, f4]);
}

#[test]
fn prime_subfield_embedding_embeds_constants() {
    let mut table = FieldTable::new();
    let f2 = table.prime_field(Integer::from_i64(2)).unwrap();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let ext = table.extension_by_field(f4).unwrap();
    let one = canonical_prime_residue(&table, f2, Integer::one()).unwrap();
    let embedded = apply_prime_subfield_embedding(&table, table.map_table(), ext.embedding, &one).unwrap();
    assert_eq!(embedded, canonical_extension_element(&table, f4, vec![Integer::one(), Integer::zero()]).unwrap());
}

#[test]
fn frobenius_on_gf4_x_gives_x_plus_one() {
    let mut table = FieldTable::new();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let spec = table.finite_field_poly_spec(f4).unwrap().owning_copy();
    let p = table.prime_modulus(f4).unwrap();
    let frob = frobenius_coords(&[Integer::zero(), Integer::one()], &spec, &p);
    assert_eq!(frob, vec![Integer::one(), Integer::one()]);
}

#[test]
fn finite_field_extension_is_galois() {
    let mut table = FieldTable::new();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let ext_id = table.extension_by_field(f4).unwrap().id;
    assert!(is_galois_extension(&table, ext_id).unwrap());
}

#[test]
fn galois_group_of_f4_over_f2_has_order_two() {
    let mut fields = FieldTable::new();
    let mut groups = GroupTable::new();
    let f4 = fields.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let ext_id = fields.extension_by_field(f4).unwrap().id;
    let req = GaloisRequest::GaloisGroupOfExtension { extension: ext_id };
    let result = execute_galois_with_tables(req, &mut fields, &mut groups);
    match result {
        GaloisResult::Exact { value: GaloisDomainValue::GaloisGroup(g) } => match g.computation {
            GaloisComputation::Complete { group } => {
                assert_eq!(groups.order(group).unwrap(), Integer::from_i64(2));
            }
            other => panic!("expected complete galois group, got {other:?}"),
        },
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn field_automorphism_applies_frobenius() {
    let mut table = FieldTable::new();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let ext_id = table.extension_by_field(f4).unwrap().id;
    let aut = field_automorphism(&mut table, ext_id, 1).unwrap();
    let x = canonical_extension_element(&table, f4, vec![Integer::zero(), Integer::one()]).unwrap();
    let fx = apply_field_automorphism(&table, table.map_table(), aut.id, &x).unwrap();
    assert_eq!(fx, canonical_extension_element(&table, f4, vec![Integer::one(), Integer::one()]).unwrap());
}

#[test]
fn extension_record_roundtrip_by_id() {
    let mut table = FieldTable::new();
    let f4 = table.polynomial_basis_field(Integer::from_i64(2), gf4_modulus()).unwrap();
    let by_field: FieldExtension = table.extension_by_field(f4).unwrap().owning_copy();
    let by_id = table.extension_record(by_field.id).unwrap();
    assert_eq!(by_id, &by_field);
}

#[test]
fn galois_requests_for_separable_and_normal() {
    let mut fields = FieldTable::new();
    let mut groups = GroupTable::new();
    let f8 =
        fields.polynomial_basis_field(Integer::from_i64(2), vec![Integer::one(), Integer::one(), Integer::zero(), Integer::one()]).unwrap();
    let ext = fields.extension_by_field(f8).unwrap().id;
    for req in [
        GaloisRequest::IsExtensionSeparable { extension: ext },
        GaloisRequest::IsExtensionNormal { extension: ext },
        GaloisRequest::IsGalois { extension: ext },
    ] {
        match execute_galois_with_tables(req, &mut fields, &mut groups) {
            GaloisResult::Exact { value: GaloisDomainValue::Boolean(true) } => {}
            other => panic!("unexpected {other:?}"),
        }
    }
}
