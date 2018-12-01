//! QS bootstrap：Fermat 近距分解（`a²−n=b²`）。

use athena_numeric::Integer;

use super::super::arithmetic::{isqrt, isqrt_if_exact};

/// Fermat 方法寻找 `n` 的非平凡因子（因子接近 `√n` 时有效）。
pub fn fermat_split(n: &Integer, max_steps: u64) -> Option<Integer> {
    if n.is_one() || n.rem(&Integer::from_i64(2)).is_zero() {
        return None;
    }
    let mut a = isqrt(n);
    if a.mul(&a).cmp(n) == std::cmp::Ordering::Less {
        a = a.add(&Integer::one());
    }
    for _ in 0..max_steps {
        let b2 = a.mul(&a).sub(n);
        if let Some(b) = isqrt_if_exact(&b2) {
            let p = a.sub(&b).gcd(n);
            if !p.is_one() && p != *n {
                return Some(p);
            }
        }
        a = a.add(&Integer::one());
    }
    None
}
