//! 素性测试 — `u64` 确定性 Miller–Rabin；更大整数固定基 → `ProbablePrime`。

use athena_numeric::Integer;

use super::{
    certificates::{CompositeWitness, PrimeCertificate, ProbablePrimeEvidence},
    value::Primality,
};
use crate::numeric_clone::clone_integer;

/// 默认 Miller–Rabin 固定基数量上限（大整数路径）。
pub const DEFAULT_MR_ROUNDS: u32 = 16;

/// 试除确定性上限（含）。
const DETERMINISTIC_TRIAL_BOUND: u64 = 1_000_000;

/// 覆盖全部 `u64` 的确定性强伪素数见证集（OEIS A014233 / Feitsma–Galway）。
const U64_DETERMINISTIC_WITNESSES: &[u64] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

/// 大整数路径的固定小素数基（可复现，**不是**独立随机样本）。
const FIXED_MR_BASES: &[u32] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

/// 素性测试。
pub fn primality_test(n: &Integer, miller_rabin_rounds: Option<u32>) -> Primality {
    if !n.is_positive() || n.is_one() {
        return Primality::Composite { witness: CompositeWitness::NonPositiveOrOne };
    }
    if n == &Integer::from_i64(2) || n == &Integer::from_i64(3) {
        return Primality::Prime { certificate: PrimeCertificate::SmallPrime };
    }
    if n.rem(&Integer::from_i64(2)).expect("rem").is_zero() {
        return Primality::Composite { witness: CompositeWitness::Even };
    }

    if let Some(small) = n.to_u64() {
        if small <= DETERMINISTIC_TRIAL_BOUND {
            return if is_prime_u64_trial(small) {
                Primality::Prime { certificate: PrimeCertificate::TrialDivision { bound: DETERMINISTIC_TRIAL_BOUND } }
            }
            else {
                Primality::Composite { witness: trial_divisor_witness_u64(small).unwrap_or(CompositeWitness::Even) }
            };
        }
        return if miller_rabin_u64_deterministic(small) {
            Primality::Prime {
                certificate: PrimeCertificate::DeterministicMillerRabin {
                    max_value_bits: 64,
                    witnesses: U64_DETERMINISTIC_WITNESSES.iter().map(|&w| w as u32).collect(),
                },
            }
        }
        else {
            Primality::Composite { witness: miller_rabin_composite_witness_u64(small).unwrap_or(CompositeWitness::Even) }
        };
    }

    if let Some(d) = smallest_factor(n) {
        return Primality::Composite { witness: CompositeWitness::SmallFactor { divisor: d } };
    }

    let requested = miller_rabin_rounds.unwrap_or(DEFAULT_MR_ROUNDS);
    if requested == 0 {
        return Primality::Unknown;
    }

    match miller_rabin_integer_fixed(n, requested) {
        MrFixedOutcome::Composite { base } => Primality::Composite { witness: CompositeWitness::MillerRabin { base } },
        MrFixedOutcome::Probable { bases } => Primality::ProbablePrime { evidence: ProbablePrimeEvidence::fixed(bases) },
    }
}

enum MrFixedOutcome {
    Composite { base: u32 },
    Probable { bases: Vec<u32> },
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

fn trial_divisor_witness_u64(n: u64) -> Option<CompositeWitness> {
    if n.is_multiple_of(2) {
        return Some(CompositeWitness::Even);
    }
    let mut i = 3u64;
    while i.saturating_mul(i) <= n {
        if n.is_multiple_of(i) {
            return Some(CompositeWitness::SmallFactor { divisor: Integer::from_u64(i) });
        }
        i += 2;
    }
    None
}

fn miller_rabin_u64_deterministic(n: u64) -> bool {
    miller_rabin_u64(n, U64_DETERMINISTIC_WITNESSES).is_ok()
}

fn miller_rabin_composite_witness_u64(n: u64) -> Option<CompositeWitness> {
    match miller_rabin_u64(n, U64_DETERMINISTIC_WITNESSES) {
        Ok(()) => None,
        Err(base) => Some(CompositeWitness::MillerRabin { base: base as u32 }),
    }
}

fn miller_rabin_u64(n: u64, witnesses: &[u64]) -> Result<(), u64> {
    if n < 2 {
        return Err(2);
    }
    let mut d = n - 1;
    let mut s = 0u32;
    while d.is_multiple_of(2) {
        d /= 2;
        s += 1;
    }
    for &a in witnesses {
        if a % n == 0 {
            continue;
        }
        let mut x = mod_pow_u64(a % n, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        let mut is_witness = true;
        for _ in 1..s {
            x = mul_mod_u64(x, x, n);
            if x == n - 1 {
                is_witness = false;
                break;
            }
        }
        if is_witness {
            return Err(a);
        }
    }
    Ok(())
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

fn smallest_factor(n: &Integer) -> Option<Integer> {
    const SMALL: &[u32] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97];
    for &p in SMALL {
        let pb = Integer::from(p);
        if n == &pb {
            return None;
        }
        if n.rem(&pb).expect("rem").is_zero() {
            return Some(pb);
        }
    }
    None
}

fn miller_rabin_integer_fixed(n: &Integer, requested_rounds: u32) -> MrFixedOutcome {
    let one = Integer::one();
    let two = Integer::from_i64(2);
    let n_minus_one = n.sub(&one);
    let mut d = clone_integer(&n_minus_one);
    let mut s = 0u32;
    while d.rem(&two).expect("rem").is_zero() {
        d = d.div(&two).expect("div");
        s += 1;
    }

    let use_rounds = (requested_rounds as usize).min(FIXED_MR_BASES.len()).max(1);
    let mut bases_used = Vec::with_capacity(use_rounds);

    for &a in &FIXED_MR_BASES[..use_rounds] {
        let base = Integer::from(a);
        if base.rem(n).expect("rem").is_zero() {
            if &base != n {
                return MrFixedOutcome::Composite { base: a };
            }
            continue;
        }
        bases_used.push(a);
        let mut x = base.mod_pow(&d, n).expect("mod_pow");
        if x == one || x == n_minus_one {
            continue;
        }
        let mut composite = true;
        for _ in 1..s {
            x = x.mod_pow(&two, n).expect("mod_pow");
            if x == n_minus_one {
                composite = false;
                break;
            }
        }
        if composite {
            return MrFixedOutcome::Composite { base: a };
        }
    }

    MrFixedOutcome::Probable { bases: bases_used }
}

/// 自 `start` 起递增枚举素数。
#[derive(Debug)]
pub struct PrimeIterator {
    current: Integer,
}

impl PrimeIterator {
    /// 从 `start`（含）起找下一个素数。
    pub fn from_start(start: impl Into<Integer>) -> Self {
        Self { current: start.into() }
    }
}

impl Iterator for PrimeIterator {
    type Item = Integer;

    fn next(&mut self) -> Option<Integer> {
        if self.current <= Integer::from_i64(2) {
            self.current = Integer::from_i64(3);
            return Some(Integer::from_i64(2));
        }
        if self.current.rem(&Integer::from_i64(2)).expect("rem").is_zero() {
            self.current = self.current.add(&Integer::one());
        }
        loop {
            if matches!(primality_test(&self.current, None), Primality::Prime { .. }) {
                let p = clone_integer(&self.current);
                self.current = self.current.add(&Integer::from_i64(2));
                return Some(p);
            }
            self.current = self.current.add(&Integer::from_i64(2));
        }
    }
}

/// Eratosthenes 筛：返回 `≤ limit` 的全部素数。
pub fn primes_up_to(limit: u64) -> Vec<Integer> {
    if limit < 2 {
        return Vec::new();
    }
    let n = limit as usize;
    let mut sieve = vec![true; n + 1];
    sieve[0] = false;
    sieve[1] = false;
    let root = (n as f64).sqrt() as usize;
    for p in 2..=root {
        if sieve[p] {
            let mut m = p * p;
            while m <= n {
                sieve[m] = false;
                m += p;
            }
        }
    }
    (2..=limit).filter(|&p| sieve[p as usize]).map(Integer::from_u64).collect()
}

/// 严格大于 `n` 的最小素数。
pub fn next_prime_after(n: &Integer) -> Integer {
    PrimeIterator::from_start(n.add(&Integer::one())).next().expect("infinite primes")
}
