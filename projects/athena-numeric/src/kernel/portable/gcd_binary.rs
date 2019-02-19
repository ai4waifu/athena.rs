//! Binary (Stein) GCD.

use std::cmp::Ordering;

use super::convenience::sub_n;
use super::primitive::{cmp_slice, is_one, is_zero, normalize_trim, trailing_zeros};
use super::shift::{shl_assign, shr_assign, shr_assign_until_odd};

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
