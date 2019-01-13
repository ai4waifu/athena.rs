//! `FieldTable` intern 测试（自 `src/algebra/table.rs` 迁出）。

use athena_engine::{FieldTable, Integer};

#[test]
fn composite_prime_field_rejected() {
    let mut table = FieldTable::new();
    let err = table.prime_field(Integer::from_i64(6)).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_MODULUS_INVALID");
}

#[test]
fn prime_field_intern_idempotent() {
    let mut table = FieldTable::new();
    let a = table.prime_field(Integer::from_i64(5)).unwrap();
    let b = table.prime_field(Integer::from_i64(5)).unwrap();
    assert_eq!(a, b);
    assert_eq!(table.characteristic(a), Some(Integer::from_i64(5)));
}
