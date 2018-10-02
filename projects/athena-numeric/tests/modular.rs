//! Modular reduction and residue tests.

use athena_numeric::{Integer, ModularValue, Modulus};

#[test]
fn reduce_and_reject_small_modulus() {
    assert!(Modulus::new(Integer::one()).is_err());
    let m = Modulus::new(Integer::from_i64(7)).expect("ok");
    assert_eq!(m.reduce(&Integer::from_i64(-1)), Integer::from_i64(6));
    let v = ModularValue::new(Integer::from_i64(10), m);
    assert_eq!(v.residue(), &Integer::from_i64(3));
}
