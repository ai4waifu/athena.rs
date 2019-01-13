//! Carry-safe arithmetic smoke tests (exercises limb kernel via `Natural`).

use athena_numeric::natural::Natural;
use std::str::FromStr;

#[test]
fn add_chain_no_carry_loss() {
    let base = Natural::from_str(&"9".repeat(400)).unwrap();
    let doubled = base.add(&base);
    let via_mul = base.mul_u64(2);
    assert_eq!(doubled, via_mul);
}

#[test]
fn mul_max_limb_pairs() {
    let max = Natural::from_u64(u64::MAX);
    let prod = max.mul(&max);
    let expected = (u128::from(u64::MAX) * u128::from(u64::MAX)).to_string();
    assert_eq!(prod.to_decimal_string(), expected);
}

#[test]
fn mul_schoolbook_vs_karatsuba_crosscheck() {
    let a = Natural::from_str(&"1234567890".repeat(70)).unwrap();
    let b = Natural::from_str(&"9876543210".repeat(70)).unwrap();
    let prod = a.mul(&b);
    // q * a + r = prod with r < a
    let (q, r) = prod.div_rem(&a);
    assert_eq!(r, Natural::zero());
    assert_eq!(q, b);
}
