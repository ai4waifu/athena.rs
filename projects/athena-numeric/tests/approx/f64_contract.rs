//! Rational exact `f64` conversion contract (see `exact::promotion` for promotion).

use athena_numeric::{Integer, Rational};

#[test]
fn rational_one_third_not_exact_f64() {
    let r = Rational::new(Integer::from_i64(1), Integer::from_i64(3));
    assert_eq!(r.try_to_f64_exact(), None);
    assert!(r.to_f64_approximate().is_some());
}

#[test]
fn rational_one_half_exact_f64() {
    let r = Rational::new(Integer::from_i64(1), Integer::from_i64(2));
    assert_eq!(r.try_to_f64_exact(), Some(0.5));
}
