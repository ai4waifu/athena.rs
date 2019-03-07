//! 复数骨架：NaN 拒绝与机器路径算术。

use athena_numeric::{BranchPolicy, Complex, Real};

#[test]
fn rejects_nan_parts() {
    let err = Complex::try_new(Real::machine(f64::NAN), Real::machine(0.0), BranchPolicy::Principal).unwrap_err();
    assert_eq!(err.details.get("operation").map(|v| v.to_string()).as_deref(), Some("complex_nan"));
}

#[test]
fn from_real_is_purely_real() {
    let z = Complex::from_real(Real::machine(3.0)).unwrap();
    assert_eq!(z.im, Real::machine(0.0));
    z.validate().unwrap();
}

#[test]
fn machine_add_mul_neg_conjugate() {
    let z = Complex::try_new(Real::machine(1.0), Real::machine(2.0), BranchPolicy::Principal).unwrap();
    let w = Complex::try_new(Real::machine(3.0), Real::machine(-4.0), BranchPolicy::Principal).unwrap();
    let sum = z.add(&w).unwrap();
    assert_eq!(sum.re, Real::machine(4.0));
    assert_eq!(sum.im, Real::machine(-2.0));

    let prod = z.mul(&w).unwrap();
    // (1+2i)(3-4i) = 3 - 4i + 6i - 8i^2 = 11 + 2i
    assert_eq!(prod.re, Real::machine(11.0));
    assert_eq!(prod.im, Real::machine(2.0));

    let n = z.neg().unwrap();
    assert_eq!(n.re, Real::machine(-1.0));
    assert_eq!(n.im, Real::machine(-2.0));

    let c = z.conjugate().unwrap();
    assert_eq!(c.re, Real::machine(1.0));
    assert_eq!(c.im, Real::machine(-2.0));
}

#[test]
fn mixed_branch_falls_back_to_principal() {
    let z = Complex::try_new(Real::machine(1.0), Real::machine(0.0), BranchPolicy::Principal).unwrap();
    let w = Complex::try_new(Real::machine(0.0), Real::machine(1.0), BranchPolicy::RealOnly).unwrap();
    let sum = z.add(&w).unwrap();
    assert_eq!(sum.branch, BranchPolicy::Principal);
}
