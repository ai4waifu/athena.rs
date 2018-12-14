//! 分解独立验证。

use athena_numeric::Integer;

use super::super::value::{CofactorStatus, FactorBaseStatus, Factorization, FactorizationCompleteness};

/// 分解验证失败原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactorizationVerifyError {
    /// 单位不是 `±1`。
    UnitInvalid,
    /// 底数 `≤ 1`。
    BaseTooSmall {
        /// 非法底。
        base: Integer,
    },
    /// 指数为零。
    ExponentZero,
    /// 因子底未严格升序。
    BasesNotSorted,
    /// 乘积与输入不一致。
    ProductMismatch {
        /// 期望（输入绝对值）。
        expected: Integer,
        /// 重建值。
        reconstructed: Integer,
    },
    /// 声称完全 / 概率完全但余因子非 1。
    CofactorNotOne,
    /// 声称确定完全但存在概率因子。
    ProbableFactorInComplete,
}

/// 验证 `|input| = |unit| * Π base^e * cofactor` 及基本规范形。
pub fn verify_factorization(input: &Integer, f: &Factorization) -> Result<(), FactorizationVerifyError> {
    if input.is_zero() {
        return Err(FactorizationVerifyError::ProductMismatch { expected: Integer::zero(), reconstructed: Integer::zero() });
    }

    let unit_abs_one = f.unit.is_one() || f.unit == Integer::from_i64(-1);
    if !unit_abs_one {
        return Err(FactorizationVerifyError::UnitInvalid);
    }

    let expected = input.abs();
    let mut product = Integer::one();
    let mut prev: Option<&Integer> = None;
    for comp in &f.factors {
        if !(comp.base > Integer::one()) {
            return Err(FactorizationVerifyError::BaseTooSmall { base: comp.base.clone() });
        }
        if comp.exponent == 0 {
            return Err(FactorizationVerifyError::ExponentZero);
        }
        if let Some(p) = prev {
            if !(comp.base > *p) {
                return Err(FactorizationVerifyError::BasesNotSorted);
            }
        }
        prev = Some(&comp.base);
        let mut power = Integer::one();
        for _ in 0..comp.exponent {
            power = power.mul(&comp.base);
        }
        product = product.mul(&power);
    }

    if !(f.cofactor >= Integer::one()) {
        return Err(FactorizationVerifyError::BaseTooSmall { base: f.cofactor.clone() });
    }

    let reconstructed = product.mul(&f.cofactor);
    if reconstructed != expected {
        return Err(FactorizationVerifyError::ProductMismatch { expected, reconstructed });
    }

    let completeness = f.completeness();
    match completeness {
        FactorizationCompleteness::Complete | FactorizationCompleteness::Probable => {
            if !f.cofactor.is_one() {
                return Err(FactorizationVerifyError::CofactorNotOne);
            }
        }
        FactorizationCompleteness::Partial | FactorizationCompleteness::ResourceLimited => {}
    }

    if completeness == FactorizationCompleteness::Complete {
        let has_probable = f.factors.iter().any(|c| matches!(c.status, FactorBaseStatus::ProbablePrime { .. }))
            || matches!(f.cofactor_status, CofactorStatus::Unknown);
        if has_probable {
            return Err(FactorizationVerifyError::ProbableFactorInComplete);
        }
    }

    Ok(())
}
