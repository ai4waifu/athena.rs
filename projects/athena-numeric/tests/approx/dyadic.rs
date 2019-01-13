//! `Dyadic` normalize and export tests (moved from `src/dyadic.rs`).

use athena_numeric::{Dyadic, integer::Sign, natural::Natural};

#[test]
fn normalize_strips_trailing_zeros() {
    let d = Dyadic::try_new(Sign::Positive, Natural::from_u64(12), 0).unwrap();
    assert_eq!(d.significand().to_u64(), Some(3));
    assert_eq!(d.exponent(), 2);
}

#[test]
fn multi_limb_rounds_to_f64() {
    let sig = Natural::from_limbs(vec![9007199254740993, 1]);
    let d = Dyadic::try_new(Sign::Positive, sig, 0).unwrap();
    assert!(d.significand_bits() > 53);
    assert!(d.to_f64_round_nearest_even().is_some());
}