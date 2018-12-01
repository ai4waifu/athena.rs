//! ECM stage 1 bootstrap（`a^k−1` 型 smooth 阶探测，与 Pollard p-1 同族）。

use athena_numeric::Integer;

/// Stage 1：对随机底 `a` 计算 `a^k−1` 与 `n` 的 gcd，`k` 为 `B1`-smooth 积。
pub fn ecm_stage_one(n: &Integer, seed: u64, b1: u32, max_curves: u32) -> Option<Integer> {
    if n.is_one() || n.rem(&Integer::from_i64(2)).is_zero() {
        return None;
    }
    let k = smooth_exponent(b1);
    for i in 0..max_curves {
        let a = Integer::from_u64(2 + (seed.wrapping_add(i as u64 * 0x517C_C1B7) % 10_000));
        let g = a.mod_pow(&k, n).sub(&Integer::one()).gcd(n);
        if !g.is_one() && g != *n {
            return Some(g);
        }
    }
    None
}

fn smooth_exponent(b1: u32) -> Integer {
    let mut k = Integer::one();
    for p in primes_up_to(b1) {
        let mut pp = p;
        while pp <= b1 {
            k = k.mul(&Integer::from_u64(p as u64));
            pp *= p;
        }
    }
    k
}

fn primes_up_to(b1: u32) -> Vec<u32> {
    if b1 < 2 {
        return Vec::new();
    }
    let mut sieve = vec![true; (b1 as usize) + 1];
    sieve[0] = false;
    sieve[1] = false;
    for p in 2..=b1 {
        if sieve[p as usize] {
            let mut m = p.saturating_mul(p);
            while m <= b1 {
                sieve[m as usize] = false;
                m += p;
            }
        }
    }
    (2..=b1).filter(|&p| sieve[p as usize]).collect()
}
