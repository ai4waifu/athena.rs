//! 整数 gcd / lcm / 扩展欧几里得。

use athena_numeric::Integer;

use super::value::ExtendedGcd;

/// 非负最大公约数；`gcd(0,0) = 0`。
pub fn gcd(a: &Integer, b: &Integer) -> Integer {
    a.gcd(b)
}

/// 非负最小公倍数；`lcm(0,0) = 0`，`lcm(0,n)=0`。
pub fn lcm(a: &Integer, b: &Integer) -> Integer {
    if a.is_zero() || b.is_zero() {
        return Integer::zero();
    }
    let g = gcd(a, b);
    a.abs().div(&g).expect("div").mul(&b.abs())
}

/// 扩展欧几里得：返回 `g = gcd(|a|,|b|)` 与 Bézout，使 `s·a + t·b = ±g` 对齐到 `s·a + t·b = g`（对原符号校正）。
pub fn extended_gcd(a: &Integer, b: &Integer) -> ExtendedGcd {
    let a_sign = if a.is_negative() { Integer::from_i64(-1) } else { Integer::one() };
    let b_sign = if b.is_negative() { Integer::from_i64(-1) } else { Integer::one() };
    let mut old_r = a.abs();
    let mut r = b.abs();
    let mut old_s = Integer::one();
    let mut s = Integer::zero();
    let mut old_t = Integer::zero();
    let mut t = Integer::one();
    while !r.is_zero() {
        let q = old_r.div(&r).expect("div");
        let next_r = old_r.sub(&q.mul(&r));
        old_r = r;
        r = next_r;
        let next_s = old_s.sub(&q.mul(&s));
        old_s = s;
        s = next_s;
        let next_t = old_t.sub(&q.mul(&t));
        old_t = t;
        t = next_t;
    }
    ExtendedGcd { g: old_r, s: old_s.mul(&a_sign), t: old_t.mul(&b_sign) }
}
