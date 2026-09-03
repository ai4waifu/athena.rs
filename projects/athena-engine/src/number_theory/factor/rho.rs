//! Pollard rho（`f(x)=x²+c`）。

use athena_numeric::Integer;
use crate::numeric_clone::{clone_integer, clone_modulus};

/// 寻找 `n` 的非平凡因子；失败返回 `None`。
pub fn pollard_rho(n: &Integer, seed: u64, c: i64, max_iters: u64) -> Option<Integer> {
    if n.is_one() {
        return None;
    }
    if n.rem(&Integer::from_i64(2)).expect("rem").is_zero() {
        return Some(Integer::from_i64(2));
    }

    let c_int = Integer::from_i64(c);
    let f = |x: &Integer| -> Integer {
        let mut y = x.mul(x).add(&c_int).rem(n).expect("rem");
        if y.is_negative() {
            y = y.add(n);
        }
        y
    };

    let mut x = Integer::from_u64(seed % 1_000_003 + 2);
    let mut y = clone_integer(&x);
    let mut d = Integer::one();
    let mut iters = 0u64;

    while d.is_one() && iters < max_iters {
        x = f(&x);
        y = f(&f(&y));
        let diff = if x >= y { x.sub(&y) } else { y.sub(&x) };
        d = diff.gcd(n);
        iters += 1;
    }

    if d.is_one() || d == *n { None } else { Some(d) }
}
