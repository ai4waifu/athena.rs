//! 素性测试 — 小整数确定；大整数 Miller-Rabin → `ProbablePrime`。

use athena_numeric::Integer;

use super::value::Primality;

/// 默认 Miller-Rabin 轮数（大整数）。
pub const DEFAULT_MR_ROUNDS: u32 = 16;

/// 试除确定性上限（含）：在此以内用试除给出 `Prime`/`Composite`。
const DETERMINISTIC_TRIAL_BOUND: u64 = 1_000_000;

/// 素性测试。
pub fn primality_test(n: &Integer, miller_rabin_rounds: Option<u32>) -> Primality {
    if !n.is_positive() || n.is_one() {
        return Primality::Composite;
    }
    if n == &Integer::from_i64(2) || n == &Integer::from_i64(3) {
        return Primality::Prime;
    }
    if n.rem(&Integer::from_i64(2)).is_zero() {
        return Primality::Composite;
    }

    if let Some(small) = n.to_u64() {
        if small <= DETERMINISTIC_TRIAL_BOUND {
            return if is_prime_u64_trial(small) { Primality::Prime } else { Primality::Composite };
        }
        return if miller_rabin_u64_deterministic(small) { Primality::Prime } else { Primality::Composite };
    }

    if has_small_factor(n) {
        return Primality::Composite;
    }

    let rounds = miller_rabin_rounds.unwrap_or(DEFAULT_MR_ROUNDS);
    if miller_rabin_integer(n, rounds) { Primality::ProbablePrime { rounds } } else { Primality::Composite }
}

fn is_prime_u64_trial(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n == 2 || n == 3 {
        return true;
    }
    if n.is_multiple_of(2) || n.is_multiple_of(3) {
        return false;
    }
    let mut i = 5u64;
    while i.saturating_mul(i) <= n {
        if n.is_multiple_of(i) || n.is_multiple_of(i + 2) {
            return false;
        }
        i = i.saturating_add(6);
    }
    true
}

fn miller_rabin_u64_deterministic(n: u64) -> bool {
    const WITNESSES: &[u64] = &[2, 3, 5, 7, 11, 13, 23];
    miller_rabin_u64(n, WITNESSES)
}

fn miller_rabin_u64(n: u64, witnesses: &[u64]) -> bool {
    if n < 2 {
        return false;
    }
    let mut d = n - 1;
    let mut s = 0u32;
    while d.is_multiple_of(2) {
        d /= 2;
        s += 1;
    }
    'next_witness: for &a in witnesses {
        if a % n == 0 {
            continue;
        }
        let mut x = mod_pow_u64(a % n, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..s {
            x = mul_mod_u64(x, x, n);
            if x == n - 1 {
                continue 'next_witness;
            }
        }
        return false;
    }
    true
}

fn mod_pow_u64(mut base: u64, mut exp: u64, m: u64) -> u64 {
    let mut result = 1u64;
    base %= m;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod_u64(result, base, m);
        }
        base = mul_mod_u64(base, base, m);
        exp >>= 1;
    }
    result
}

fn mul_mod_u64(a: u64, b: u64, m: u64) -> u64 {
    ((a as u128 * b as u128) % m as u128) as u64
}

fn has_small_factor(n: &Integer) -> bool {
    const SMALL: &[u32] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97];
    for &p in SMALL {
        let pb = Integer::from(p);
        if n == &pb {
            return false;
        }
        if n.rem(&pb).is_zero() {
            return true;
        }
    }
    false
}

fn miller_rabin_integer(n: &Integer, rounds: u32) -> bool {
    let one = Integer::one();
    let two = Integer::from_i64(2);
    let n_minus_one = n.sub(&one);
    let mut d = n_minus_one.clone();
    let mut s = 0u32;
    while d.rem(&two).is_zero() {
        d = d.div(&two);
        s += 1;
    }
    const BASES: &[u32] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    let use_rounds = (rounds as usize).min(BASES.len()).max(1);
    for &a in &BASES[..use_rounds] {
        let base = Integer::from(a);
        if base.rem(n).is_zero() {
            continue;
        }
        let mut x = base.mod_pow(&d, n);
        if x == one || x == n_minus_one {
            continue;
        }
        let mut composite = true;
        for _ in 1..s {
            x = x.mod_pow(&two, n);
            if x == n_minus_one {
                composite = false;
                break;
            }
        }
        if composite {
            return false;
        }
    }
    true
}
