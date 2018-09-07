//! 整数试除因式分解（bootstrap）。

use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};

use super::{
    primes::primality_test,
    value::{Factorization, FactorizationCompleteness, Primality, PrimePower},
};

/// 分解资源上限。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactorLimits {
    /// 试除上界（含）；默认 `10^6`。
    pub max_trial: u64,
    /// 输入绝对值比特上限；超出 → `ResourceLimited`。
    pub max_bits: u32,
}

impl Default for FactorLimits {
    fn default() -> Self {
        Self {
            max_trial: 1_000_000,
            max_bits: 256,
        }
    }
}

/// 整数因式分解（试除 bootstrap）。
pub fn factor_integer(n: &BigInt, limits: &FactorLimits) -> Factorization {
    if n.is_zero() {
        return Factorization {
            unit: BigInt::one(),
            factors: Vec::new(),
            remainder: BigInt::zero(),
            completeness: FactorizationCompleteness::Partial,
        };
    }

    let unit = if n.is_negative() { BigInt::from(-1) } else { BigInt::one() };
    let mut m = n.abs();

    if m.is_one() {
        return Factorization {
            unit,
            factors: Vec::new(),
            remainder: BigInt::one(),
            completeness: FactorizationCompleteness::Complete,
        };
    }

    if m.bits() > u64::from(limits.max_bits) {
        return Factorization {
            unit,
            factors: Vec::new(),
            remainder: m,
            completeness: FactorizationCompleteness::ResourceLimited,
        };
    }

    let mut factors: Vec<PrimePower> = Vec::new();

    // 因子 2
    let two = BigInt::from(2);
    let mut exp2 = 0u32;
    while (&m % &two).is_zero() {
        m /= &two;
        exp2 += 1;
    }
    if exp2 > 0 {
        factors.push(PrimePower { base: two, exponent: exp2 });
    }

    let mut p: u64 = 3;
    let trial_cap = limits.max_trial;
    while p <= trial_cap {
        let pb = BigInt::from(p);
        // p*p > m → 剩余为 1 或素数
        if let (Some(ps), Some(ms)) = (pb.to_u128(), m.to_u128()) {
            if ps.saturating_mul(ps) > ms && m > BigInt::one() {
                break;
            }
        } else if &pb * &pb > m && m > BigInt::one() {
            break;
        }

        let mut e = 0u32;
        while (&m % &pb).is_zero() {
            m /= &pb;
            e += 1;
        }
        if e > 0 {
            factors.push(PrimePower {
                base: pb,
                exponent: e,
            });
        }
        p = p.saturating_add(2);
        if p > trial_cap {
            break;
        }
    }

    if m.is_one() {
        return Factorization {
            unit,
            factors,
            remainder: BigInt::one(),
            completeness: FactorizationCompleteness::Complete,
        };
    }

    // 剩余可能为素数 / 合数 / 超出试除
    match primality_test(&m, None) {
        Primality::Prime => {
            factors.push(PrimePower {
                base: m,
                exponent: 1,
            });
            Factorization {
                unit,
                factors,
                remainder: BigInt::one(),
                completeness: FactorizationCompleteness::Complete,
            }
        }
        Primality::ProbablePrime { .. } => {
            factors.push(PrimePower {
                base: m,
                exponent: 1,
            });
            Factorization {
                unit,
                factors,
                remainder: BigInt::one(),
                completeness: FactorizationCompleteness::Probable,
            }
        }
        Primality::Composite | Primality::Unknown => {
            let completeness = if m.bits() > 40 {
                FactorizationCompleteness::ResourceLimited
            } else {
                FactorizationCompleteness::Partial
            };
            Factorization {
                unit,
                factors,
                remainder: m,
                completeness,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factor_12() {
        let f = factor_integer(&12.into(), &FactorLimits::default());
        assert_eq!(f.completeness, FactorizationCompleteness::Complete);
        assert_eq!(f.unit, BigInt::one());
        assert_eq!(f.factors.len(), 2);
        assert_eq!(f.factors[0].base, BigInt::from(2));
        assert_eq!(f.factors[0].exponent, 2);
        assert_eq!(f.factors[1].base, BigInt::from(3));
        assert_eq!(f.factors[1].exponent, 1);
    }

    #[test]
    fn factor_negative() {
        let f = factor_integer(&(-100).into(), &FactorLimits::default());
        assert_eq!(f.unit, BigInt::from(-1));
        assert_eq!(f.completeness, FactorizationCompleteness::Complete);
    }
}
