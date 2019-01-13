//! 带显式工作精度的二进制任意精度实数。
//!
//! [`Decimal`] 存储**已舍入**的浮点载荷：尾数宽度不得超过 [`Self::precision_bits`]。
//! 无精度合同时的精确二元有理数请用 [`Dyadic`]。
//!
//! 舍入在 [`Natural`] limb 上完成（guard / round / sticky），**不得**经 IEEE binary64 中转。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{dyadic::Dyadic, integer::Sign, natural::Natural, rounding::RoundingPolicy};

/// 允许的最小工作精度（至少一个尾数位）。
pub const MIN_PRECISION_BITS: u32 = 1;

/// IEEE binary64 尾数宽度（含隐含位）。
pub const IEEE754_BINARY64_PRECISION: u32 = 53;

/// 对 [`Decimal`] 舍入操作的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoundingStatus {
    /// 未丢弃信息；相对无限精度载荷值不变。
    Exact,
    /// 舍入使幅度增大（绝对值上远离零，朝 ±∞）。
    RoundedUp,
    /// 舍入使幅度减小（绝对值上朝向零）。
    RoundedDown,
    /// 已丢弃信息但方向未分类（保留 / 定向平局）。
    Inexact,
}

/// 有限精度二进制浮点（`Dyadic` 载荷 + 声明精度）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decimal {
    dyadic: Dyadic,
    precision_bits: u32,
}

impl Decimal {
    /// 最小工作精度下的规范零（`+0`）。
    ///
    /// 须保留调用方工作精度时优先用 [`Self::zero_with_precision`]。
    pub fn zero() -> Self {
        Self { dyadic: Dyadic::zero(), precision_bits: MIN_PRECISION_BITS }
    }

    /// 指定工作精度（`≥ 1`）下的规范零（`+0`）。
    pub fn zero_with_precision(precision_bits: u32) -> Result<Self> {
        if precision_bits < MIN_PRECISION_BITS {
            return Err(invalid("precision_zero"));
        }
        Ok(Self { dyadic: Dyadic::zero(), precision_bits })
    }

    /// 当尾数落入 `precision_bits` 时包装精确 [`Dyadic`]。
    pub fn try_from_dyadic(dyadic: Dyadic, precision_bits: u32) -> Result<Self> {
        if precision_bits < MIN_PRECISION_BITS {
            return Err(invalid("precision_zero"));
        }
        dyadic.validate()?;
        if !dyadic.is_zero() {
            let bits = dyadic.significand_bits();
            if bits > u64::from(precision_bits) {
                return Err(precision_exceeds(bits, precision_bits));
            }
        }
        Ok(Self { dyadic, precision_bits })
    }

    /// 由原始部件构造（精确二元有理须落入精度）。
    pub fn try_new(sign: Sign, significand: Natural, exponent: i64, precision_bits: u32) -> Result<Self> {
        let dyadic = Dyadic::try_new(sign, significand, exponent)?;
        Self::try_from_dyadic(dyadic, precision_bits)
    }

    /// 导入有限 `f64`，诚实使用 53 位工作精度。
    pub fn from_f64(x: f64) -> Result<Self> {
        let dyadic = Dyadic::from_f64(x)?;
        Self::try_from_dyadic(dyadic, IEEE754_BINARY64_PRECISION)
    }

    /// 精确二元有理载荷。
    pub fn dyadic(&self) -> &Dyadic {
        &self.dyadic
    }

    /// 符号。
    pub fn sign(&self) -> Sign {
        self.dyadic.sign()
    }

    /// 无符号尾数幅度。
    pub fn significand(&self) -> &Natural {
        self.dyadic.significand()
    }

    /// 二进制指数。
    pub fn exponent(&self) -> i64 {
        self.dyadic.exponent()
    }

    /// 声明的工作精度（位）。
    pub fn precision_bits(&self) -> u32 {
        self.precision_bits
    }

    /// 是否恰为零。
    pub fn is_zero(&self) -> bool {
        self.dyadic.is_zero()
    }

    /// 是否恰为 `+1`。
    pub fn is_one(&self) -> bool {
        self.dyadic.is_one()
    }

    /// 去掉精确载荷末尾的二进制零。
    pub fn normalize(&mut self) {
        self.dyadic.normalize();
        if let Err(e) = self.validate() {
            panic!("BigFloat invariant broken after normalize: {:?}", e);
        }
    }

    /// 校验规范不变量与精度合同。
    pub fn validate(&self) -> Result<()> {
        if self.precision_bits < MIN_PRECISION_BITS {
            return Err(invalid("precision_zero"));
        }
        self.dyadic.validate()?;
        if !self.dyadic.is_zero() && self.dyadic.significand_bits() > u64::from(self.precision_bits) {
            return Err(precision_exceeds(self.dyadic.significand_bits(), self.precision_bits));
        }
        Ok(())
    }

    /// 载荷可精确表示时导出为 `f64`。
    pub fn to_f64_exact(&self) -> Option<f64> {
        self.dyadic.to_f64_exact()
    }

    /// 舍入到最近 IEEE binary64（平局取偶）。
    pub fn to_f64_round_nearest_even(&self) -> Option<f64> {
        self.dyadic.to_f64_round_nearest_even()
    }

    /// 舍入到最近偶后的有损 `f64`。
    pub fn to_f64_approximate(&self) -> Option<f64> {
        self.to_f64_round_nearest_even()
    }

    /// 将载荷舍入到新工作精度（最近偶，limb 内核）。
    pub fn round_to_precision(&self, precision_bits: u32) -> Result<(Self, RoundingStatus)> {
        self.round_to_precision_with_mode(precision_bits, RoundingPolicy::NearestEven)
    }

    /// 在显式舍入策略下将载荷舍入到新工作精度。
    pub fn round_to_precision_with_mode(&self, precision_bits: u32, mode: RoundingPolicy) -> Result<(Self, RoundingStatus)> {
        if precision_bits < MIN_PRECISION_BITS {
            return Err(invalid("precision_zero"));
        }
        if self.dyadic.is_zero() {
            return Ok((Self { dyadic: Dyadic::zero(), precision_bits }, RoundingStatus::Exact));
        }
        let bits = self.dyadic.significand_bits();
        if bits <= u64::from(precision_bits) {
            return Ok((Self { dyadic: self.dyadic.clone(), precision_bits }, RoundingStatus::Exact));
        }

        let discard = bits - u64::from(precision_bits);
        let sig = self.dyadic.significand();
        let mut truncated = sig.shr_bits(discard);
        let round_bit = discard > 0 && sig.bit(discard - 1);
        let sticky = discard > 1 && sig.any_bits_below(discard - 1);
        let lsb = truncated.bit(0);
        let positive = self.dyadic.sign() != Sign::Negative;

        let round_up = match mode {
            RoundingPolicy::NearestEven => round_bit && (sticky || lsb),
            RoundingPolicy::TowardZero => false,
            RoundingPolicy::TowardPosInf => {
                if positive {
                    round_bit || sticky
                }
                else {
                    false
                }
            }
            RoundingPolicy::TowardNegInf => {
                if positive {
                    false
                }
                else {
                    round_bit || sticky
                }
            }
        };

        let mut status = if round_up {
            RoundingStatus::RoundedUp
        }
        else if round_bit || sticky {
            RoundingStatus::RoundedDown
        }
        else {
            RoundingStatus::Exact
        };

        if round_up {
            truncated = truncated.add_u64(1);
            // 进位超出 p 位窗口：`1 << precision_bits`。
            if truncated.bits() > u64::from(precision_bits) {
                truncated = truncated.shr_bits(1);
                let exp = self
                    .dyadic
                    .exponent()
                    .checked_add(discard as i64)
                    .and_then(|e| e.checked_add(1))
                    .ok_or_else(|| invalid("exponent_overflow"))?;
                let dyadic = Dyadic::try_new(self.dyadic.sign(), truncated, exp)?;
                let value = Self::try_from_dyadic(dyadic, precision_bits)?;
                return Ok((value, status));
            }
        }

        // 负值朝 ±∞：绝对值意义上幅度增大已是 RoundedUp。
        // 有符号定向模式下，对负值朝 +∞ 截断时重映射状态：
        if matches!(mode, RoundingPolicy::TowardPosInf | RoundingPolicy::TowardNegInf) && !positive && (round_bit || sticky) {
            // 负 + TowardPosInf 朝零截断 → 幅度下降 → RoundedDown
            // 负 + TowardNegInf 远离零舍入 → 幅度上升 → RoundedUp
            status = match mode {
                RoundingPolicy::TowardPosInf => RoundingStatus::RoundedDown,
                RoundingPolicy::TowardNegInf => {
                    if round_up {
                        RoundingStatus::RoundedUp
                    }
                    else {
                        RoundingStatus::RoundedDown
                    }
                }
                _ => status,
            };
        }

        let exp = self.dyadic.exponent().checked_add(discard as i64).ok_or_else(|| invalid("exponent_overflow"))?;
        let dyadic = Dyadic::try_new(self.dyadic.sign(), truncated, exp)?;
        let value = Self::try_from_dyadic(dyadic, precision_bits)?;
        Ok((value, status))
    }
}

fn invalid(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", op)
}

fn precision_exceeds(bits: u64, precision_bits: u32) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericPrecisionLoss)
        .detail("domain", "numeric")
        .detail("operation", "precision_exceeds")
        .detail("significand_bits", bits.to_string())
        .detail("precision_bits", precision_bits.to_string())
}
