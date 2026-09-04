//! 区间不变量与定向算术包络测试。

use athena_numeric::{Interval, IntervalDecoration, Real};

#[test]
fn rejects_inverted_bounds() {
    let err = Interval::try_bounded(Real::machine(2.0), Real::machine(1.0), IntervalDecoration::Trivial).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");
}

#[test]
fn rejects_nan_endpoints() {
    let err = Interval::try_bounded(Real::machine(f64::NAN), Real::machine(1.0), IntervalDecoration::Trivial).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");
}

#[test]
fn full_unbounded_promotes_to_entire() {
    let iv = Interval::try_bounded(Real::machine(f64::NEG_INFINITY), Real::machine(f64::INFINITY), IntervalDecoration::Trivial).unwrap();
    assert!(iv.is_entire());
}

#[test]
fn point_interval() {
    let iv = Interval::try_point(Real::machine(3.0)).unwrap();
    assert!(iv.contains_f64(3.0).unwrap());
    assert!(!iv.contains_f64(4.0).unwrap());
}

#[test]
fn add_encloses_true_sum() {
    let a = Interval::try_bounded(Real::machine(1.0_f64), Real::machine(1.0_f64.next_up()), IntervalDecoration::Trivial).unwrap();
    let b = Interval::try_bounded(Real::machine(2.0_f64), Real::machine(2.0_f64), IntervalDecoration::Trivial).unwrap();
    let sum = a.add(&b).unwrap();
    let (lo, hi) = sum.as_f64_bounds().unwrap();
    let target = 3.0_f64.next_up();
    assert!(lo <= target);
    assert!(hi >= target);
}

#[test]
fn mul_widens_product() {
    let a = Interval::try_bounded(Real::machine(1.1), Real::machine(1.1), IntervalDecoration::Trivial).unwrap();
    let b = Interval::try_bounded(Real::machine(1.1), Real::machine(1.1), IntervalDecoration::Trivial).unwrap();
    let prod = a.mul(&b).unwrap();
    let (lo, hi) = prod.as_f64_bounds().unwrap();
    let exact = 1.1_f64 * 1.1_f64;
    assert!(lo <= exact);
    assert!(hi >= exact);
}

#[test]
fn neg_swaps_and_flips_bounds() {
    let iv = Interval::try_bounded(Real::machine(-1.0), Real::machine(2.5), IntervalDecoration::Certain).unwrap();
    let n = iv.neg().unwrap();
    let (lo, hi) = n.as_f64_bounds().unwrap();
    assert_eq!(lo, -2.5);
    assert_eq!(hi, 1.0);
    assert!(!iv.is_point());
    assert!(Interval::try_point(Real::machine(3.0)).unwrap().is_point());
}

#[test]
fn div_by_interval_containing_zero_is_error() {
    let a = Interval::try_bounded(Real::machine(1.0), Real::machine(2.0), IntervalDecoration::Trivial).unwrap();
    let b = Interval::try_bounded(Real::machine(-1.0), Real::machine(1.0), IntervalDecoration::Trivial).unwrap();
    let err = a.div(&b).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");
}
