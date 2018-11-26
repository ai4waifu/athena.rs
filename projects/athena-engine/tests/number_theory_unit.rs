//! Direct unit tests for number-theory primitives (gcd, factor, modular, primes).

use std::str::FromStr;

use athena_engine::{
    CompositeWitness, FactorBaseStatus, FactorLimits, FactorizationCompleteness, Integer, MillerRabinBaseSelection, Modulus,
    Primality, PrimeModulus, extended_gcd, factor_integer, gcd, mod_inverse, mod_pow, primality_test, verify_factorization,
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
    assert_eq!(f.completeness(), FactorizationCompleteness::Complete);
    assert_eq!(f.unit, Integer::one());
    assert_eq!(f.factors.len(), 2);
    assert_eq!(f.factors[0].base, Integer::from_i64(2));
    assert_eq!(f.factors[0].exponent, 2);
    assert!(matches!(f.factors[0].status, FactorBaseStatus::ProvenPrime { .. }));
    assert_eq!(f.factors[1].base, Integer::from_i64(3));
    assert_eq!(f.factors[1].exponent, 1);
    verify_factorization(&12.into(), &f).expect("verify 12");
}

#[test]
fn factor_negative() {
    let f = factor_integer(&(-100).into(), &FactorLimits::default()).expect("factor -100");
    assert_eq!(f.unit, Integer::from_i64(-1));
    assert_eq!(f.completeness(), FactorizationCompleteness::Complete);
    verify_factorization(&(-100).into(), &f).expect("verify -100");
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
fn prime_modulus_from_proven() {
    let p = primality_test(&97.into(), None);
    assert!(matches!(p, Primality::Prime { .. }));
    let pm = PrimeModulus::assuming_proven(97).expect("prime modulus");
    assert_eq!(pm.value(), &Integer::from_i64(97));
}

#[test]
fn small_primes_and_composite_witnesses() {
    assert!(matches!(primality_test(&2.into(), None), Primality::Prime { .. }));
    assert!(matches!(primality_test(&97.into(), None), Primality::Prime { .. }));
    match primality_test(&91.into(), None) {
        Primality::Composite {
            witness: CompositeWitness::SmallFactor { divisor },
        } => assert_eq!(divisor, Integer::from_i64(7)),
        other => panic!("91 should be composite with small factor: {other:?}"),
    }
    assert!(matches!(
        primality_test(&1.into(), None),
        Primality::Composite {
            witness: CompositeWitness::NonPositiveOrOne
        }
    ));
}

#[test]
fn u64_deterministic_covers_large_prime() {
    let n = Integer::from_u64(u64::MAX - 58);
    assert!(matches!(primality_test(&n, None), Primality::Prime { .. }));
}

#[test]
fn u64_deterministic_rejects_composite() {
    let n = Integer::from_u64(1_000_003u64 * 1_000_003u64);
    assert!(matches!(primality_test(&n, None), Primality::Composite { .. }));
}

#[test]
fn probable_prime_metadata_matches_executed_bases() {
    let n = Integer::from_str("1050134283599687836074166315548193334825931101974830645589").expect("parse");
    match primality_test(&n, Some(1000)) {
        Primality::ProbablePrime { evidence } => {
            assert_eq!(evidence.base_selection, MillerRabinBaseSelection::Fixed);
            assert_eq!(evidence.rounds_executed, evidence.bases.len() as u32);
            assert!(evidence.rounds_executed <= 12);
            assert_ne!(evidence.rounds_executed, 1000);
            assert_eq!(primality_test(&n, Some(0)), Primality::Unknown);
        }
        Primality::Composite { .. } => {}
        other => panic!("expected ProbablePrime or Composite, got {other:?}"),
    }
}

#[test]
fn probable_rounds_capped_to_actual() {
    let n = Integer::from_str("1050134283599687836074166315548193334825931101974830645589").expect("parse");
    if let Primality::ProbablePrime { evidence } = primality_test(&n, Some(3)) {
        assert_eq!(evidence.rounds_executed, 3);
        assert_eq!(evidence.bases, vec![2, 3, 5]);
    }
}

#[test]
fn factor_limits_policy_budget() {
    let limits = FactorLimits::default();
    assert_eq!(limits.max_trial(), 1_000_000);
    assert_eq!(limits.max_bits(), 256);
}
