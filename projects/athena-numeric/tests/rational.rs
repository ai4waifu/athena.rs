//! Rational cross-cancellation and numeric ordering tests.

use std::cmp::Ordering;

use athena_numeric::{Integer, Rational};

#[test]
fn add_cross_cancels_common_denominator_factor() {
    let million = Integer::from_i64(1_000_000);
    let a = Rational::new(Integer::from_i64(1), million.clone());
    let b = Rational::new(Integer::from_i64(1), million);
    assert_eq!(a.add(&b), Rational::new(Integer::from_i64(1), Integer::from_i64(500_000)));
}

#[test]
fn mul_cross_cancels_before_product() {
    let a = Rational::new(Integer::from_i64(12), Integer::from_i64(35));
    let b = Rational::new(Integer::from_i64(25), Integer::from_i64(18));
    assert_eq!(a.mul(&b), Rational::new(Integer::from_i64(10), Integer::from_i64(21)));
}

#[test]
fn div_cross_cancels_before_product() {
    let a = Rational::new(Integer::from_i64(12), Integer::from_i64(35));
    let b = Rational::new(Integer::from_i64(18), Integer::from_i64(25));
    assert_eq!(a.try_div(&b).unwrap(), Rational::new(Integer::from_i64(10), Integer::from_i64(21)));
}

#[test]
fn cmp_numeric_is_mathematical_not_lexicographic() {
    let two_thirds = Rational::new(Integer::from_i64(2), Integer::from_i64(3));
    let three_hundredths = Rational::new(Integer::from_i64(3), Integer::from_i64(100));
    assert_eq!(two_thirds.cmp_numeric(&three_hundredths), Ordering::Greater);
    assert_eq!(three_hundredths.cmp_numeric(&two_thirds), Ordering::Less);
    let half = Rational::new(Integer::from_i64(1), Integer::from_i64(2));
    let two_over_four = Rational::new(Integer::from_i64(2), Integer::from_i64(4));
    assert_eq!(half.cmp_numeric(&two_over_four), Ordering::Equal);
}
