//! Limb arithmetic coverage via public `Natural` / budget APIs.
//!
//! Direct `limb_kernel` unit tests lived under `src/` and are rewritten here so the
//! kernel module stays crate-private.

use athena_numeric::{ExecutionBudget, NumericBackendLimits, natural::Natural};
use std::str::FromStr;

fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state
}

fn random_natural(state: &mut u64, max_limbs: usize) -> Natural {
    let n = (lcg_next(state) as usize % max_limbs) + 1;
    let limbs: Vec<u64> = (0..n).map(|_| lcg_next(state)).collect();
    Natural::from_limbs(limbs)
}

#[test]
fn budget_rejects_mul_that_would_exceed_max_limbs() {
    let budget = ExecutionBudget::from_limits(&NumericBackendLimits {
        max_limbs: Some(2),
        max_significand_bits: None,
        max_wire_payload_bytes: None,
        max_pow_exp: None,
    });
    let err = budget.check_mul(3, 3).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_RESOURCE_LIMIT");
}

#[test]
fn mul_matches_div_rem_identity() {
    let mut seed = 0x4710_u64;
    for _ in 0..32 {
        let a = random_natural(&mut seed, 40);
        let b = random_natural(&mut seed, 40);
        if b.is_zero() {
            continue;
        }
        let prod = a.mul(&b);
        let (q, r) = prod.div_rem(&a);
        assert!(r.is_zero());
        assert_eq!(q, b);
    }
}

#[test]
fn sqr_matches_mul_self() {
    let mut seed = 0x5A00_u64;
    for _ in 0..64 {
        let a = random_natural(&mut seed, 40);
        assert_eq!(a.sqr(), a.mul(&a));
    }
}

#[test]
fn gcd_matches_euclidean_reference() {
    let mut seed = 0x6CD2_u64;
    for _ in 0..64 {
        let a = random_natural(&mut seed, 24);
        let b = random_natural(&mut seed, 24);
        let g = a.gcd(&b);
        let mut x = a.clone();
        let mut y = b.clone();
        while !y.is_zero() {
            let (_, r) = x.div_rem(&y);
            x = y;
            y = r;
        }
        assert_eq!(g, x);
    }
}

#[test]
fn mod_pow_small_matches_binary() {
    let base = Natural::from_u64(3);
    let exp = Natural::from_u64(13);
    let modulus = Natural::from_u64(17);
    assert_eq!(base.mod_pow(&exp, &modulus), Natural::from_u64(12));
}

#[test]
fn large_decimal_roundtrip_mul() {
    let a = Natural::from_str(&"1234567890".repeat(40)).unwrap();
    let b = Natural::from_str(&"9876543210".repeat(40)).unwrap();
    let prod = a.mul(&b);
    let (q, r) = prod.div_rem(&a);
    assert!(r.is_zero());
    assert_eq!(q, b);
}
