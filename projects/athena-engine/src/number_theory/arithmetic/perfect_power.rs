//! 完全幂检测。

use athena_numeric::Integer;

use super::isqrt::isqrt;
use crate::numeric_clone::clone_integer;

/// 若 `n = b^e`（`e > 1`）返回 `(base, exponent)`，否则 `None`（`n > 1`）。
pub fn perfect_power_decomposition(n: &Integer) -> Option<(Integer, u32)> {
    if n.is_one() || n.is_zero() {
        return None;
    }
    let abs = n.abs();
    if abs.is_one() {
        return None;
    }

    let max_exp = abs.bits().min(64) as u32;
    for exp in (2..=max_exp).rev() {
        if let Some(base) = integer_root(&abs, exp) {
            if base.pow_u32(exp).ok().as_ref() == Some(&abs) {
                let signed = if n.is_negative() && exp % 2 == 1 {
                    base.neg()
                }
                else if n.is_negative() {
                    continue;
                }
                else {
                    base
                };
                return Some((signed, exp));
            }
        }
    }
    None
}

/// 是否完全幂（`n = b^e`，`e > 1`）。
pub fn is_perfect_power(n: &Integer) -> bool {
    perfect_power_decomposition(n).is_some()
}

fn integer_root(n: &Integer, exp: u32) -> Option<Integer> {
    if n.is_zero() {
        return Some(Integer::zero());
    }
    if exp == 1 {
        return Some(clone_integer(&n));
    }
    if exp == 2 {
        return Some(isqrt(n));
    }
    let mut lo = Integer::zero();
    let mut hi = isqrt(n).add(&Integer::one());
    while lo.add(&Integer::one()) < hi {
        let mid = lo.add(&hi).div(&Integer::from_i64(2)).expect("div");
        match mid.pow_u32(exp) {
            Ok(p) if p <= *n => lo = mid,
            Ok(_) => hi = mid,
            Err(_) => hi = mid,
        }
    }
    if lo.pow_u32(exp).ok().as_ref() == Some(n) { Some(lo) } else { None }
}
