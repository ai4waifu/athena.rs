//! 有理数重构（Wang / 扩展欧几里得）。

use athena_numeric::{Integer, Modulus, Rational};

use super::super::value::{RationalReconstruction, RationalReconstructionFailure};

/// 在模 `m` 下从剩余 `r` 重构分数 `n/d`，要求 `|n| ≤ N`、`0 < d ≤ D` 且既约。
///
/// 默认界（`N`/`D` 为 `None`）取 `⌊√(m/2)⌋`。
pub fn rational_reconstruction(
    residue: &Integer,
    modulus: &Modulus,
    max_numerator: Option<&Integer>,
    max_denominator: Option<&Integer>,
) -> RationalReconstruction {
    let m = modulus.value();
    if !(m > &Integer::one()) {
        return RationalReconstruction::NotFound {
            reason: RationalReconstructionFailure::InvalidBounds,
        };
    }

    let half = m.div(&Integer::from_i64(2));
    let default_bound = if half.is_zero() {
        isqrt(&Integer::one())
    } else {
        isqrt(&half)
    };
    let n_bound = max_numerator.cloned().unwrap_or_else(|| default_bound.clone());
    let d_bound = max_denominator.cloned().unwrap_or(default_bound);

    if n_bound.is_negative() || d_bound.is_negative() || d_bound.is_zero() {
        return RationalReconstruction::NotFound {
            reason: RationalReconstructionFailure::InvalidBounds,
        };
    }

    let r = modulus.reduce(residue);
    let mut old_r = m.clone();
    let mut rem = r.clone();
    let mut old_t = Integer::zero();
    let mut t = Integer::one();

    while !rem.is_zero() {
        if rem.abs() <= n_bound && t.abs() <= d_bound && !t.is_zero() {
            let (numer, denom) = if t.is_negative() {
                (rem.neg(), t.neg())
            } else {
                (rem.clone(), t.clone())
            };
            match Rational::try_new(numer, denom) {
                Ok(value) => return RationalReconstruction::Found { value },
                Err(_) => {
                    return RationalReconstruction::NotFound {
                        reason: RationalReconstructionFailure::NoCandidate,
                    };
                }
            }
        }
        let q = old_r.div(&rem);
        let next_r = old_r.sub(&q.mul(&rem));
        old_r = rem;
        rem = next_r;
        let next_t = old_t.sub(&q.mul(&t));
        old_t = t;
        t = next_t;
    }

    if r.abs() <= n_bound {
        if let Ok(value) = Rational::try_new(r, Integer::one()) {
            return RationalReconstruction::Found { value };
        }
    }

    RationalReconstruction::NotFound {
        reason: RationalReconstructionFailure::NoCandidate,
    }
}

fn isqrt(n: &Integer) -> Integer {
    if n.is_zero() || n.is_one() {
        return n.clone();
    }
    if n.is_negative() {
        return Integer::zero();
    }
    let bits = n.bits();
    let mut x = Integer::one();
    for _ in 0..((bits + 1) / 2) {
        x = x.mul(&Integer::from_i64(2));
    }
    if x.is_zero() {
        x = Integer::one();
    }
    loop {
        let y = x.add(&n.div(&x)).div(&Integer::from_i64(2));
        if y >= x {
            while x.mul(&x) > *n {
                x = x.sub(&Integer::one());
            }
            return x;
        }
        x = y;
    }
}
