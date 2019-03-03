//! # Purpose
//! Half-GCD (Jebelean / Lehmer-matrix style) for wide non-negative limb magnitudes.
//!
//! # Mathematical model
//! Euclidean transforms preserve $\gcd(a,b)$. A unimodular $2\times 2$ matrix
//! accumulated from leading-limb quotients is applied as signed linear
//! combinations. Half-GCD stops once the smaller operand is about half the
//! original width, then finishes with ordinary Lehmer/binary GCD.
//!
//! # Derivation
//! Leading double-limb Euclid mirrors full Euclid while the quotient sequence
//! is certified; the matrix product is then exact on the full operands.
//! Negative matrix entries are required (unlike the conservative non-negative
//! Lehmer path). When certification fails, one exact remainder restores
//! progress.
//!
//! # Algorithm steps
//! 1. Normalize; swap so $a \ge b$.
//! 2. While $\min(|a|,|b|)$ is at least the half-GCD threshold, reduce toward
//!    half width via signed Lehmer blocks (or one exact rem on failure).
//! 3. Finish with [`super::gcd_lehmer::gcd`].
//!
//! # Preconditions
//! - Canonical non-negative little-endian `$u64$` limbs (`Vec` convenience path).
//!
//! # Postconditions
//! - Returns $\gcd(a,b)$ as a canonical limb vector.
//!
//! # Complexity
//! Fewer full-precision divisions than plain Euclid on wide inputs; asymptotic
//! still dominated by the finishing GCD.
//!
//! # Crossover
//! Planner selects HalfGcd when both operands have at least
//! `GCD_LEHMER_THRESHOLD * 4` limbs and `half_gcd` capability is set.
//!
//! # Failure modes
//! A Lehmer block may return false (unstable leading quotients); the outer loop
//! then performs one exact `div_rem`.
//!
//! # Tests
//! `tests/exact/algorithms.rs` half-GCD capability cross-check.

use std::cmp::Ordering;

use super::convenience::{add_n, div_rem, mul_1, sub_n};
use super::gcd_lehmer::gcd as lehmer_gcd;
use super::primitive::{cmp_slice, effective_len, is_zero, normalize_trim};

/// Match planner threshold (`GCD_LEHMER_THRESHOLD * 4`).
const HALF_GCD_THRESHOLD: usize = 12;
const LEHMER_THRESHOLD: usize = 3;

/// Half-GCD then Lehmer/binary finish.
pub(crate) fn half_gcd(mut a: Vec<u64>, mut b: Vec<u64>) -> Vec<u64> {
    a = normalize_trim(a);
    b = normalize_trim(b);
    if is_zero(&a) {
        return b;
    }
    if is_zero(&b) {
        return a;
    }
    if cmp_slice(&a, &b) == Ordering::Less {
        std::mem::swap(&mut a, &mut b);
    }

    while effective_len(&b) >= HALF_GCD_THRESHOLD && effective_len(&a) >= HALF_GCD_THRESHOLD {
        let n0 = effective_len(&a).max(effective_len(&b));
        let target = (n0 + 1) / 2;
        let mut progressed = false;
        while effective_len(&b) > target.max(LEHMER_THRESHOLD) {
            if hgcd_lehmer_block(&mut a, &mut b) {
                a = normalize_trim(a);
                b = normalize_trim(b);
                progressed = true;
            }
            else {
                // Exact Euclidean step when leading prediction fails.
                let (_q, r) = div_rem(&a, &b);
                a = b;
                b = normalize_trim(r);
                progressed = true;
                break;
            }
            if is_zero(&b) {
                return a;
            }
            if cmp_slice(&a, &b) == Ordering::Less {
                std::mem::swap(&mut a, &mut b);
            }
        }
        if !progressed {
            break;
        }
        if is_zero(&b) {
            return a;
        }
        if cmp_slice(&a, &b) == Ordering::Less {
            std::mem::swap(&mut a, &mut b);
        }
    }

    lehmer_gcd(a, b)
}

/// One signed-matrix Lehmer block (Jebelean-style HGCD step fragment).
fn hgcd_lehmer_block(a: &mut Vec<u64>, b: &mut Vec<u64>) -> bool {
    let na = effective_len(a);
    let nb = effective_len(b);
    if nb < 2 || na < nb {
        return false;
    }
    let n = na - nb;

    let u1 = *a.get(nb + n - 1).unwrap_or(&0);
    let u0 = *a.get(nb + n - 2).unwrap_or(&0);
    let v1 = b[nb - 1];
    let v0 = if nb >= 2 { b[nb - 2] } else { 0 };
    if v1 == 0 {
        return false;
    }

    let mut x0: i64 = 1;
    let mut x1: i64 = 0;
    let mut y0: i64 = 0;
    let mut y1: i64 = 1;
    let mut uh = (u128::from(u1) << 64) | u128::from(u0);
    let mut vh = (u128::from(v1) << 64) | u128::from(v0);
    let mut steps = 0u32;

    while vh >= (1u128 << 63) {
        let q = uh / vh;
        let r = uh % vh;
        if r < (1u128 << 63) {
            break;
        }
        let t = x0 as i128 - (q as i128) * (x1 as i128);
        if t < i64::MIN as i128 || t > i64::MAX as i128 {
            break;
        }
        x0 = x1;
        x1 = t as i64;
        let t = y0 as i128 - (q as i128) * (y1 as i128);
        if t < i64::MIN as i128 || t > i64::MAX as i128 {
            break;
        }
        y0 = y1;
        y1 = t as i64;
        uh = vh;
        vh = r;
        steps += 1;
        if steps > 64 {
            break;
        }
    }

    if steps == 0 || y1 == 0 {
        return false;
    }
    if y1.unsigned_abs() > u32::MAX as u64 || x1.unsigned_abs() > u32::MAX as u64 {
        return false;
    }

    let Some(na_new) = lincomb_signed(x0, a, x1, b) else {
        return false;
    };
    let Some(nb_new) = lincomb_signed(y0, a, y1, b) else {
        return false;
    };
    if is_zero(&nb_new) {
        *a = na_new;
        *b = nb_new;
        return true;
    }
    // Require non-increasing smaller operand for progress.
    if effective_len(&nb_new) >= nb && cmp_slice(&nb_new, b) != Ordering::Less {
        return false;
    }
    *a = na_new;
    *b = nb_new;
    if cmp_slice(a, b) == Ordering::Less {
        std::mem::swap(a, b);
    }
    true
}

/// $|c_0|·v_0 \pm |c_1|·v_1$ as a non-negative magnitude (signs choose add/sub).
fn lincomb_signed(c0: i64, v0: &[u64], c1: i64, v1: &[u64]) -> Option<Vec<u64>> {
    let zero = || vec![0u64];
    let mag = |c: i64, v: &[u64]| -> Vec<u64> {
        if c == 0 {
            zero()
        }
        else {
            mul_1(v, c.unsigned_abs())
        }
    };
    let t0 = mag(c0, v0);
    let t1 = mag(c1, v1);
    let s0 = c0 >= 0;
    let s1 = c1 >= 0;
    Some(match (s0, s1) {
        (true, true) => add_n(&t0, &t1),
        (false, false) => add_n(&t0, &t1),
        (true, false) => {
            if cmp_slice(&t0, &t1) == Ordering::Less {
                sub_n(&t1, &t0)
            }
            else {
                sub_n(&t0, &t1)
            }
        }
        (false, true) => {
            if cmp_slice(&t1, &t0) == Ordering::Less {
                sub_n(&t0, &t1)
            }
            else {
                sub_n(&t1, &t0)
            }
        }
    })
}
