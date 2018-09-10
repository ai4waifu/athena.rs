//! 整数 gcd / lcm / 扩展欧几里得。

use num_bigint::BigInt;
use num_traits::{Signed, Zero};

use super::value::ExtendedGcd;

/// 非负最大公约数；`gcd(0,0) = 0`。
pub fn gcd(a: &BigInt, b: &BigInt) -> BigInt {
    let mut a = a.abs();
    let mut b = b.abs();
    while !b.is_zero() {
        let r = &a % &b;
        a = b;
        b = r;
    }
    a
}

/// 非负最小公倍数；`lcm(0,0) = 0`，`lcm(0,n)=0`。
pub fn lcm(a: &BigInt, b: &BigInt) -> BigInt {
    if a.is_zero() || b.is_zero() {
        return BigInt::zero();
    }
    let g = gcd(a, b);
    (a.abs() / g) * b.abs()
}

/// 扩展欧几里得：返回 `g = gcd(|a|,|b|)` 与 Bézout，使 `s·a + t·b = ±g` 对齐到 `s·a + t·b = g`（对原符号校正）。
pub fn extended_gcd(a: &BigInt, b: &BigInt) -> ExtendedGcd {
    let a_sign = if a.is_negative() { BigInt::from(-1) } else { BigInt::from(1) };
    let b_sign = if b.is_negative() { BigInt::from(-1) } else { BigInt::from(1) };
    let mut old_r = a.abs();
    let mut r = b.abs();
    let mut old_s = BigInt::from(1);
    let mut s = BigInt::zero();
    let mut old_t = BigInt::zero();
    let mut t = BigInt::from(1);
    while !r.is_zero() {
        let q = &old_r / &r;
        let next_r = &old_r - &q * &r;
        old_r = r;
        r = next_r;
        let next_s = &old_s - &q * &s;
        old_s = s;
        s = next_s;
        let next_t = &old_t - &q * &t;
        old_t = t;
        t = next_t;
    }
    ExtendedGcd { g: old_r, s: old_s * a_sign, t: old_t * b_sign }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcd_basic() {
        assert_eq!(gcd(&12.into(), &18.into()), BigInt::from(6));
        assert_eq!(gcd(&(-12).into(), &18.into()), BigInt::from(6));
        assert_eq!(gcd(&0.into(), &0.into()), BigInt::zero());
    }

    #[test]
    fn egcd_bezout() {
        let a = BigInt::from(240);
        let b = BigInt::from(46);
        let e = extended_gcd(&a, &b);
        assert_eq!(&e.s * &a + &e.t * &b, e.g);
        assert_eq!(e.g, BigInt::from(2));
    }
}
