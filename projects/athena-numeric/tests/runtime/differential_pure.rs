//! 使用 crate 内参考算法的随机差分测试（无 `num-*` oracle）。

use athena_numeric::{Integer, natural::Natural};
use std::str::FromStr;

fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state
}

fn random_decimal_digits(state: &mut u64, max_digits: usize) -> String {
    let len = (lcg_next(state) as usize % max_digits) + 1;
    let mut s = String::with_capacity(len);
    for i in 0..len {
        let d = if i == 0 { (lcg_next(state) % 9 + 1) as u8 } else { (lcg_next(state) % 10) as u8 };
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

fn reference_gcd(mut x: Natural, mut y: Natural) -> Natural {
    while !y.is_zero() {
        let (_, r) = x.div_rem(&y);
        x = y;
        y = r;
    }
    x
}

fn reference_mod_pow(base: Natural, exp: Natural, modulus: &Natural) -> Natural {
    if modulus.is_one() {
        return Natural::zero();
    }
    let mut result = Natural::one();
    let mut b = base.div_rem(modulus).1;
    let mut e = exp;
    while !e.is_zero() {
        let (e_half, r) = e.div_rem(&Natural::from_u64(2));
        if !r.is_zero() {
            result = result.mul(&b).div_rem(modulus).1;
        }
        b = b.mul(&b).div_rem(modulus).1;
        e = e_half;
    }
    result
}

#[test]
fn natural_add_sub_mul_algebraic_identities() {
    let mut seed = 0xD1FF_u64;
    for _ in 0..128 {
        let a = Natural::from_str(&random_decimal_digits(&mut seed, 80)).unwrap();
        let b = Natural::from_str(&random_decimal_digits(&mut seed, 80)).unwrap();
        assert_eq!(a.add(&b), b.add(&a));
        assert_eq!(a.mul(&b), b.mul(&a));
        let sum = a.add(&b);
        assert_eq!(sum.sub(&b), a);
        if a >= b {
            assert_eq!(a.sub(&b).add(&b), a);
        }
        assert_eq!(a.mul(&Natural::one()), a);
        assert_eq!(a.mul(&Natural::zero()), Natural::zero());
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

        let (q, r) = a.div_rem(&b);
        assert!(r < b);
        assert_eq!(q.mul(&b).add(&r), a);
    }
}

#[test]
fn natural_gcd_matches_euclidean_reference() {
    let mut seed = 0x6CD0_u64;
    for _ in 0..64 {
        let a = Natural::from_str(&random_decimal_digits(&mut seed, 64)).unwrap();
        let b = Natural::from_str(&random_decimal_digits(&mut seed, 64)).unwrap();
        assert_eq!(a.gcd(&b), reference_gcd(a.clone(), b.clone()));
    }
}

#[test]
fn natural_mod_pow_matches_reference() {
    let primes = ["7", "97", "1009", "65537"];
    let mut seed = 0xF000_u64;
    for _ in 0..48 {
        let base = Natural::from_str(&random_decimal_digits(&mut seed, 24)).unwrap();
        let exp = Natural::from_str(&random_decimal_digits(&mut seed, 12)).unwrap();
        let p_str = primes[(lcg_next(&mut seed) as usize) % primes.len()];
        let modulus = Natural::from_str(p_str).unwrap();
        assert_eq!(base.mod_pow(&exp, &modulus), reference_mod_pow(base, exp, &modulus));
    }
}

#[test]
fn integer_signed_ops_algebraic_identities() {
    let mut seed = 0x1A00_u64;
    for _ in 0..96 {
        let a = Integer::from_str(&random_signed_decimal(&mut seed, 48)).unwrap();
        let b = Integer::from_str(&random_signed_decimal(&mut seed, 48)).unwrap();
        assert_eq!(a.add(&b), b.add(&a));
        assert_eq!(a.mul(&b), b.mul(&a));
        assert_eq!(a.add(&b).sub(&b), a);
        if !b.is_zero() {
            assert_eq!(a.div(&b).unwrap().mul(&b).add(&a.rem(&b).unwrap()), a);
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
fn integer_gcd_matches_euclidean_reference() {
    let mut seed = 0x6CD1_u64;
    for _ in 0..64 {
        let a = Integer::from_str(&random_signed_decimal(&mut seed, 64)).unwrap();
        let b = Integer::from_str(&random_signed_decimal(&mut seed, 64)).unwrap();
        let am = Natural::from_str(&a.abs().to_decimal_string()).unwrap();
        let bm = Natural::from_str(&b.abs().to_decimal_string()).unwrap();
        let g = reference_gcd(am, bm);
        assert_eq!(a.gcd(&b).abs().to_decimal_string(), g.to_decimal_string());
    }
}

#[test]
fn sqr_matches_mul_reference() {
    let mut seed = 0x5A00_u64;
    for _ in 0..64 {
        let a = Natural::from_str(&random_decimal_digits(&mut seed, 56)).unwrap();
        assert_eq!(a.sqr(), a.mul(&a));
    }
}
