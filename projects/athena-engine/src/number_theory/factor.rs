//! 整数试除因式分解（bootstrap）。

use athena_numeric::Integer;
use athena_types::Diagnostic;

use super::{
    primes::primality_test,
    result::factor_zero_invalid,
    value::{Factorization, FactorizationCompleteness, Primality, PrimePower},
};

/// 分解资源上限。
///
/// Gate 1 仅有试除界与比特上限。比特超限表示**当前 bootstrap 拒绝执行**，
/// 不是已消耗的 wall/CPU 预算；完整 `FactorPolicy` / `ExecutionBudget` 见 Living `08`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactorLimits {
    /// 试除上界（含）；默认 `10^6`。
    pub max_trial: u64,
    /// 输入绝对值比特上限；超出 → `ResourceLimited`（bootstrap 拒绝，非耗尽预算）。
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
///
/// `0` 无有限素因数分解 → `Err`（域错误），不得用 `Partial` 假装未分完。
pub fn factor_integer(n: &Integer, limits: &FactorLimits) -> Result<Factorization, Diagnostic> {
    if n.is_zero() {
        return Err(factor_zero_invalid());
    }

    let unit = if n.is_negative() {
        Integer::from_i64(-1)
    } else {
        Integer::one()
    };
    let mut m = n.abs();

    if m.is_one() {
        return Ok(Factorization {
            unit,
            factors: Vec::new(),
            remainder: Integer::one(),
            completeness: FactorizationCompleteness::Complete,
        });
    }

    if m.bits() > u64::from(limits.max_bits) {
        return Ok(Factorization {
            unit,
            factors: Vec::new(),
            remainder: m,
            completeness: FactorizationCompleteness::ResourceLimited,
        });
    }

    let mut factors: Vec<PrimePower> = Vec::new();

    let two = Integer::from_i64(2);
    let mut exp2 = 0u32;
    while m.rem(&two).is_zero() {
        m = m.div(&two);
        exp2 += 1;
    }
    if exp2 > 0 {
        factors.push(PrimePower {
            base: two,
            exponent: exp2,
        });
    }

    let mut p: u64 = 3;
    let trial_cap = limits.max_trial;
    while p <= trial_cap {
        let pb = Integer::from_u64(p);
        if let (Some(ps), Some(ms)) = (pb.to_u128(), m.to_u128()) {
            if ps.saturating_mul(ps) > ms && m > Integer::one() {
                break;
            }
        } else if pb.mul(&pb) > m && m > Integer::one() {
            break;
        }

        let mut e = 0u32;
        while m.rem(&pb).is_zero() {
            m = m.div(&pb);
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
        return Ok(Factorization {
            unit,
            factors,
            remainder: Integer::one(),
            completeness: FactorizationCompleteness::Complete,
        });
    }

    match primality_test(&m, None) {
        Primality::Prime => {
            factors.push(PrimePower {
                base: m,
                exponent: 1,
            });
            Ok(Factorization {
                unit,
                factors,
                remainder: Integer::one(),
                completeness: FactorizationCompleteness::Complete,
            })
        }
        Primality::ProbablePrime { .. } => {
            factors.push(PrimePower {
                base: m,
                exponent: 1,
            });
            Ok(Factorization {
                unit,
                factors,
                remainder: Integer::one(),
                completeness: FactorizationCompleteness::Probable,
            })
        }
        Primality::Composite | Primality::Unknown => {
            let completeness = if m.bits() > 40 {
                FactorizationCompleteness::ResourceLimited
            } else {
                FactorizationCompleteness::Partial
            };
            Ok(Factorization {
                unit,
                factors,
                remainder: m,
                completeness,
            })
        }
    }
}
