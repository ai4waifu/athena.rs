//! # Purpose
//! Binary (Stein) GCD without division.
//!
//! # Mathematical model
//! $\gcd(2a,2b)=2\gcd(a,b)$, $\gcd(2a,b)=\gcd(a,b)$ for odd $b$, and
//! $\gcd(a,b)=\gcd(|a-b|,\min(a,b))$ for odd positives.
//!
//! # Derivation
//! Factor out shared powers of two, then subtract odd values and strip new factors
//! of two until equality; restore the common shift.
//!
//! # Algorithm steps
//! 1. Handle zeros; record $\min(v_2(a),v_2(b))$.
//! 2. Make both odd; loop subtract / swap / shift until equal.
//! 3. Left-shift by the saved common valuation.
//!
//! # Preconditions
//! - Canonical non-negative magnitudes.
//!
//! # Postconditions
//! - Canonical $\gcd$.
//!
//! # Complexity
//! Bit complexity favorable when operands share small factors of two; can lag
//! Lehmer on wide random odds.
//!
//! # Crossover
//! Used as the terminal path of gcd and for smaller widths.
//!
//! # Failure modes
//! None beyond empty/zero handling.
//!
//! # Tests
//! Euclidean reference suites in `tests/exact/` and `tests/runtime/differential_pure.rs`.

use std::cmp::Ordering;

use super::{
    convenience::sub_n,
    primitive::{cmp_slice, is_one, is_zero, normalize_trim, trailing_zeros},
    shift::{shl_assign, shr_assign, shr_assign_until_odd},
};

/// Binary GCD (Stein's algorithm).
///
/// Remove common powers of two, then repeatedly subtract the smaller odd value
/// from the larger and strip newly exposed factors of two. Subtraction preserves
/// gcd for ordered positive values, and shifts only remove powers of two. The
/// saved common shift is restored at the end. This avoids division but can be
/// inferior to Lehmer on wide random inputs. Inputs are canonical magnitudes.
pub(crate) fn binary_gcd(mut a: Vec<u64>, mut b: Vec<u64>) -> Vec<u64> {
    a = normalize_trim(a);
    b = normalize_trim(b);
    if is_zero(&a) {
        return b;
    }
    if is_zero(&b) {
        return a;
    }

    let shift = trailing_zeros(&a).min(trailing_zeros(&b));
    shr_assign(&mut a, shift);
    shr_assign(&mut b, shift);

    loop {
        shr_assign_until_odd(&mut a);
        shr_assign_until_odd(&mut b);
        if cmp_slice(&a, &b) == Ordering::Equal {
            break;
        }
        if cmp_slice(&a, &b) == Ordering::Less {
            std::mem::swap(&mut a, &mut b);
        }
        if is_one(&b) {
            break;
        }
        a = sub_n(&a, &b);
        shr_assign(&mut a, 1);
    }
    a = b;
    shl_assign(&mut a, shift);
    normalize_trim(a)
}
