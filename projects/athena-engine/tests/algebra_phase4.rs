//! 代数父对象 Phase 4：ℚ / 𝔽_p 元素 canonical 化与显式 embedding。

use athena_engine::{
    FieldElementRepr, FieldRequest, FieldTable, Integer, add_field_elements, apply_field_embedding,
    canonical_prime_residue, canonical_rational, execute_field_with_table_mut, mul_field_elements,
};
use athena_types::DiagnosticCode;

#[test]
fn rational_canonical_reduces_fraction() {
    let mut table = FieldTable::new();
    let q = table.rationals();
    let e = canonical_rational(&table, q, Integer::from_i64(6), Integer::from_i64(4)).unwrap();
    match &e.repr {
        FieldElementRepr::Rational { value } => {
            assert_eq!(value.numerator(), Integer::from_i64(3));
            assert_eq!(value.denominator(), Integer::from_i64(2));
        }
        _ => panic!("expected rational repr"),
    }
}

#[test]
fn prime_residue_canonical_in_range() {
    let mut table = FieldTable::new();
    let f7 = table.prime_field(Integer::from_i64(7)).unwrap();
    let e = canonical_prime_residue(&table, f7, Integer::from_i64(-1)).unwrap();
    assert_eq!(
        e.repr,
        FieldElementRepr::PrimeFieldResidue { value: Integer::from_i64(6) }
    );
}

#[test]
fn canonical_embedding_q_to_fp_is_idempotent() {
    let mut table = FieldTable::new();
    let q = table.rationals();
    let f5 = table.prime_field(Integer::from_i64(5)).unwrap();
    let m1 = table.canonical_embedding_rationals_to_prime(f5).unwrap();
    let m2 = table.canonical_embedding_rationals_to_prime(f5).unwrap();
    assert_eq!(m1, m2);
    assert_eq!(table.map_table().canonical_embedding(q, f5), Some(m1));
}

#[test]
fn embed_rational_via_explicit_map() {
    let mut table = FieldTable::new();
    let q = table.rationals();
    let f5 = table.prime_field(Integer::from_i64(5)).unwrap();
    let map = table.canonical_embedding_rationals_to_prime(f5).unwrap();
    let half = canonical_rational(&table, q, Integer::from_i64(1), Integer::from_i64(2)).unwrap();
    let embedded = apply_field_embedding(&table, table.map_table(), map, &half).unwrap();
    // 1/2 mod 5 = 1 * 2^{-1} = 1 * 3 = 3
    assert_eq!(
        embedded.repr,
        FieldElementRepr::PrimeFieldResidue { value: Integer::from_i64(3) }
    );
}

#[test]
fn embed_fails_when_denominator_not_invertible_mod_p() {
    let mut table = FieldTable::new();
    let q = table.rationals();
    let f5 = table.prime_field(Integer::from_i64(5)).unwrap();
    let map = table.canonical_embedding_rationals_to_prime(f5).unwrap();
    let one_fifth = canonical_rational(&table, q, Integer::from_i64(1), Integer::from_i64(5)).unwrap();
    let err = apply_field_embedding(&table, table.map_table(), map, &one_fifth).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_MODULAR_INVERSE_MISSING");
}

#[test]
fn field_add_mul_on_q_and_fp() {
    let mut table = FieldTable::new();
    let q = table.rationals();
    let f5 = table.prime_field(Integer::from_i64(5)).unwrap();

    let a = canonical_rational(&table, q, Integer::from_i64(1), Integer::from_i64(3)).unwrap();
    let b = canonical_rational(&table, q, Integer::from_i64(1), Integer::from_i64(6)).unwrap();
    let sum = add_field_elements(&table, &a, &b).unwrap();
    match &sum.repr {
        FieldElementRepr::Rational { value } => {
            assert_eq!(value.numerator(), Integer::from_i64(1));
            assert_eq!(value.denominator(), Integer::from_i64(2));
        }
        _ => panic!("expected rational"),
    }

    let x = canonical_prime_residue(&table, f5, Integer::from_i64(3)).unwrap();
    let y = canonical_prime_residue(&table, f5, Integer::from_i64(4)).unwrap();
    let prod = mul_field_elements(&table, &x, &y).unwrap();
    assert_eq!(prod.repr, FieldElementRepr::PrimeFieldResidue { value: Integer::from_i64(2) });
}

#[test]
fn execute_field_with_table_mut_registers_fields() {
    let mut table = FieldTable::new();
    let r = execute_field_with_table_mut(FieldRequest::Rationals, &mut table);
    assert!(matches!(r, athena_engine::FieldResult::Exact { .. }));
    let r = execute_field_with_table_mut(
        FieldRequest::PrimeField { characteristic: Integer::from_i64(11) },
        &mut table,
    );
    assert!(matches!(r, athena_engine::FieldResult::Exact { .. }));
}

#[test]
fn field_mismatch_on_add() {
    let mut table = FieldTable::new();
    let q = table.rationals();
    let f5 = table.prime_field(Integer::from_i64(5)).unwrap();
    let a = canonical_rational(&table, q, Integer::one(), Integer::one()).unwrap();
    let b = canonical_prime_residue(&table, f5, Integer::one()).unwrap();
    let err = add_field_elements(&table, &a, &b).unwrap_err();
    assert_eq!(err.code.as_str(), DiagnosticCode::FieldMismatch.as_str());
}
