//! Pollard p−1（第 1 阶段）。

use athena_numeric::Integer;

/// 第 1 阶段：`a^{M(B1)} ≡ 1 (mod p)` 当 `p−1` 为 `B1`-smooth 时，返回非平凡因子。
pub fn pollard_p1(n: &Integer, seed: u64, b1: u32) -> Option<Integer> {
    if n.is_one() || !n.is_odd() {
        return None;
    }
    let mut a = Integer::from_u64(2 + (seed % 10_000));
    if a.rem(n).expect("rem").is_zero() {
        a = a.add(&Integer::one());
    }
    a = a.rem(n).expect("rem");
    if a.is_zero() || a.is_one() {
        a = Integer::from_i64(2);
    }

    let m = smooth_exponent(b1);
    let am = a.mod_pow(&m, n).expect("mod_pow");
    let g = am.sub(&Integer::one()).gcd(n);
    if !g.is_one() && g != *n { Some(g) } else { None }
}

fn smooth_exponent(b1: u32) -> Integer {
    let mut k = Integer::one();
    for p in primes_up_to(b1) {
        let mut pp = p;
        while pp <= b1 {
            k = k.mul(&Integer::from_u64(u64::from(p)));
            let next = pp.saturating_mul(p);
            if next <= pp {
                break;
            }
            pp = next;
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
    let limit = b1 as usize;
    for p in 2..=limit {
        if sieve[p] {
            let mut m = p.saturating_mul(p);
            while m <= limit {
                sieve[m] = false;
                m += p;
            }
        }
    }
    (2..=b1).filter(|&p| sieve[p as usize]).collect()
}
