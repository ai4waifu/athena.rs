//! Direct unit tests for number-theory primitives (gcd, factor, modular, primes).

use athena_engine::{
    FactorLimits, FactorizationCompleteness, Integer, Modulus, Primality, extended_gcd, factor_integer, gcd,
    mod_inverse, mod_pow, primality_test,
};
use athena_types::DiagnosticCode;

#[test]
fn gcd_basic() {
    assert_eq!(gcd(&12.into(), &18.into()), Integer::from_i64(6));
    assert_eq!(gcd(&(-12).into(), &18.into()), Integer::from_i64(6));
    assert_eq!(gcd(&0.into(), &0.into()), Integer::zero());
}

#[test]
fn egcd_bezout() {
    let a = Integer::from_i64(240);
    let b = Integer::from_i64(46);
    let e = extended_gcd(&a, &b);
    assert_eq!(e.s.mul(&a).add(&e.t.mul(&b)), e.g);
    assert_eq!(e.g, Integer::from_i64(2));
}

#[test]
fn factor_12() {
    let f = factor_integer(&12.into(), &FactorLimits::default()).expect("factor 12");
    assert_eq!(f.completeness, FactorizationCompleteness::Complete);
    assert_eq!(f.unit, Integer::one());
    assert_eq!(f.factors.len(), 2);
    assert_eq!(f.factors[0].base, Integer::from_i64(2));
    assert_eq!(f.factors[0].exponent, 2);
    assert_eq!(f.factors[1].base, Integer::from_i64(3));
    assert_eq!(f.factors[1].exponent, 1);
}

#[test]
fn factor_negative() {
    let f = factor_integer(&(-100).into(), &FactorLimits::default()).expect("factor -100");
    assert_eq!(f.unit, Integer::from_i64(-1));
    assert_eq!(f.completeness, FactorizationCompleteness::Complete);
}

#[test]
fn factor_zero_is_domain_error() {
    let err = factor_integer(&Integer::zero(), &FactorLimits::default()).expect_err("zero");
    assert_eq!(err.code, DiagnosticCode::DomainError);
}

#[test]
fn inverse_and_pow() {
    let m = Modulus::new(7).unwrap();
    let inv = mod_inverse(&3.into(), &m).unwrap();
    assert_eq!(inv.residue(), &Integer::from_i64(5));
    let p = mod_pow(&3.into(), &4.into(), &m).unwrap();
    assert_eq!(p.residue(), &Integer::from_i64(4)); // 81 ≡ 4 (mod 7)
}

#[test]
fn small_primes() {
    assert_eq!(primality_test(&2.into(), None), Primality::Prime);
    assert_eq!(primality_test(&97.into(), None), Primality::Prime);
    assert_eq!(primality_test(&91.into(), None), Primality::Composite);
    assert_eq!(primality_test(&1.into(), None), Primality::Composite);
}
