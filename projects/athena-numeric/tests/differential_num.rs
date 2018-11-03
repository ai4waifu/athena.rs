//! Differential and property tests against `num-bigint` (test-only reference, not public API).

use athena_numeric::{Integer, natural::Natural};
use num_bigint::{BigInt, BigUint};
use num_integer::Integer as NumInteger;
use std::str::FromStr;

fn natural_to_biguint(n: &Natural) -> BigUint {
    BigUint::from_str(&n.to_decimal_string()).unwrap()
}

fn integer_to_bigint(n: &Integer) -> BigInt {
    BigInt::from_str(&n.to_decimal_string()).unwrap()
}

fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state
}

fn random_decimal_digits(state: &mut u64, max_digits: usize) -> String {
    let len = (lcg_next(state) as usize % max_digits) + 1;
    let mut s = String::with_capacity(len);
    for i in 0..len {
        let d = if i == 0 {
            (lcg_next(state) % 9 + 1) as u8
        } else {
            (lcg_next(state) % 10) as u8
        };
        s.push(char::from(b'0' + d));
    }
    s
}

fn random_signed_decimal(state: &mut u64, max_digits: usize) -> String {
    let neg = lcg_next(state) & 1 == 0;
    let mut s = random_decimal_digits(state, max_digits);
    if neg && s != "0" {
        s.insert(0, '-');
    }
    s
}

#[test]
fn natural_add_sub_mul_match_num_bigint() {
    let mut seed = 0xD1FF_u64;
    for _ in 0..128 {
        let sa = random_decimal_digits(&mut seed, 80);
        let sb = random_decimal_digits(&mut seed, 80);
        let a = Natural::from_str(&sa).unwrap();
        let b = Natural::from_str(&sb).unwrap();
        let ref_a = natural_to_biguint(&a);
        let ref_b = natural_to_biguint(&b);

        assert_eq!(natural_to_biguint(&a.add(&b)), &ref_a + &ref_b);
        if a >= b {
            assert_eq!(natural_to_biguint(&a.sub(&b)), &ref_a - &ref_b);
        }
        assert_eq!(natural_to_biguint(&a.mul(&b)), &ref_a * &ref_b);
    }
}

#[test]
fn natural_div_rem_identity_and_reference() {
    let mut seed = 0xD100_u64;
    for _ in 0..96 {
        let sa = random_decimal_digits(&mut seed, 72);
        let sb = random_decimal_digits(&mut seed, 48);
        let mut b = Natural::from_str(&sb).unwrap();
        if b.is_zero() {
            b = Natural::one();
        }
        let a = Natural::from_str(&sa).unwrap();
        let ref_a = natural_to_biguint(&a);
        let ref_b = natural_to_biguint(&b);

        let (q, r) = a.div_rem(&b);
        let ref_q = &ref_a / &ref_b;
        let ref_r = &ref_a % &ref_b;
        assert_eq!(natural_to_biguint(&q), ref_q);
        assert_eq!(natural_to_biguint(&r), ref_r);
        assert!(r < b);
        assert_eq!(q.mul(&b).add(&r), a);
    }
}

#[test]
fn natural_gcd_matches_num_bigint() {
    let mut seed = 0x6CD0_u64;
    for _ in 0..64 {
        let a = Natural::from_str(&random_decimal_digits(&mut seed, 64)).unwrap();
        let b = Natural::from_str(&random_decimal_digits(&mut seed, 64)).unwrap();
        let ref_a = natural_to_biguint(&a);
        let ref_b = natural_to_biguint(&b);
        assert_eq!(natural_to_biguint(&a.gcd(&b)), NumInteger::gcd(&ref_a, &ref_b));
    }
}

#[test]
fn natural_mod_pow_matches_num_bigint() {
    let primes = ["7", "97", "1009", "65537"];
    let mut seed = 0xF000_u64;
    for _ in 0..48 {
        let base = Natural::from_str(&random_decimal_digits(&mut seed, 24)).unwrap();
        let exp = Natural::from_str(&random_decimal_digits(&mut seed, 12)).unwrap();
        let p_str = primes[(lcg_next(&mut seed) as usize) % primes.len()];
        let modulus = Natural::from_str(p_str).unwrap();
        let ref_base = natural_to_biguint(&base);
        let ref_exp = natural_to_biguint(&exp);
        let ref_mod = natural_to_biguint(&modulus);
        let ref_out = ref_base.modpow(&ref_exp, &ref_mod);
        assert_eq!(natural_to_biguint(&base.mod_pow(&exp, &modulus)), ref_out);
    }
}

#[test]
fn integer_signed_ops_match_num_bigint() {
    let mut seed = 0x1A00_u64;
    for _ in 0..96 {
        let sa = random_signed_decimal(&mut seed, 48);
        let sb = random_signed_decimal(&mut seed, 48);
        let a = Integer::from_str(&sa).unwrap();
        let b = Integer::from_str(&sb).unwrap();
        let ref_a = integer_to_bigint(&a);
        let ref_b = integer_to_bigint(&b);

        assert_eq!(integer_to_bigint(&a.add(&b)), &ref_a + &ref_b);
        assert_eq!(integer_to_bigint(&a.sub(&b)), &ref_a - &ref_b);
        assert_eq!(integer_to_bigint(&a.mul(&b)), &ref_a * &ref_b);
        if !b.is_zero() {
            assert_eq!(integer_to_bigint(&a.div(&b)), &ref_a / &ref_b);
            assert_eq!(integer_to_bigint(&a.rem(&b)), &ref_a % &ref_b);
        }
    }
}

#[test]
fn mod_small_prime_congruence_property() {
    let primes: [u64; 8] = [3, 5, 7, 11, 13, 17, 19, 23];
    let mut seed = 0xA0D0_u64;
    for _ in 0..128 {
        let a = Natural::from_str(&random_decimal_digits(&mut seed, 40)).unwrap();
        let b = Natural::from_str(&random_decimal_digits(&mut seed, 40)).unwrap();
        let p = primes[(lcg_next(&mut seed) as usize) % primes.len()];
        let m = Natural::from_u64(p);
        let prod = a.mul(&b);
        let (_, ra) = a.div_rem(&m);
        let (_, rb) = b.div_rem(&m);
        let (_, rp) = prod.div_rem(&m);
        let lhs = ra.mul(&rb).div_rem(&m).1;
        assert_eq!(lhs, rp, "mod {p} congruence failed");
    }
}

#[test]
fn integer_gcd_matches_num_bigint() {
    let mut seed = 0x6CD1_u64;
    for _ in 0..64 {
        let a = Integer::from_str(&random_signed_decimal(&mut seed, 64)).unwrap();
        let b = Integer::from_str(&random_signed_decimal(&mut seed, 64)).unwrap();
        let ref_a = integer_to_bigint(&a);
        let ref_b = integer_to_bigint(&b);
        assert_eq!(integer_to_bigint(&a.gcd(&b)), NumInteger::gcd(&ref_a, &ref_b));
    }
}

#[test]
fn sqr_matches_mul_reference() {
    let mut seed = 0x5A00_u64;
    for _ in 0..64 {
        let s = random_decimal_digits(&mut seed, 56);
        let a = Natural::from_str(&s).unwrap();
        assert_eq!(natural_to_biguint(&a.sqr()), natural_to_biguint(&a.mul(&a)));
    }
}
