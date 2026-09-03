//! 整数平方根（数学 floor `√n`）。

use athena_numeric::Integer;
use crate::numeric_clone::{clone_integer};

/// `⌊√n⌋`（`n ≥ 0`；负输入返回 `0`）。
pub fn isqrt(n: &Integer) -> Integer {
    if n.is_zero() || n.is_one() {
        return clone_integer(&n);
    }
    if n.is_negative() {
        return Integer::zero();
    }
    let mut x = clone_integer(&n);
    let two = Integer::from_i64(2);
    loop {
        let y = x.add(&n.div(&x).expect("div")).div(&two).expect("div");
        if y >= x {
            while x.mul(&x).cmp(n) == std::cmp::Ordering::Greater {
                x = x.sub(&Integer::one());
            }
            return x;
        }
        x = y;
    }
}

/// 若 `n = r²` 则返回 `Some(r)`，否则 `None`（`n ≥ 0`）。
pub fn isqrt_if_exact(n: &Integer) -> Option<Integer> {
    if n.is_negative() {
        return None;
    }
    let r = isqrt(n);
    if r.mul(&r) == *n { Some(r) } else { None }
}
