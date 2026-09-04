//! 带显式工作精度的二进制任意精度实数。
//!
//! [`Decimal`] 存储**已舍入**的浮点载荷：尾数宽度不得超过 [`Self::precision_bits`]。
//! 无精度合同时的精确二元有理数请用 [`Dyadic`]。
//!
//! 布局：自有 `significand_meta + Magnitude` + `exponent` + `precision_bits`
//! （禁止 `Decimal → Dyadic → Natural` 套娃）。
//! 舍入在 [`Natural`] limb 上完成（guard / round / sticky），**不得**经 IEEE binary64 中转。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    dyadic::Dyadic,
    execution_budget::NumericContext,
    integer::Sign,
    natural::Natural,
    rounding::RoundingPolicy,
    storage::{MagnitudePair, gc_alloc_error},
};

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

/// 有限精度二进制浮点（自有 significand Magnitude + 声明精度）。
// Living 19: no Clone on Heap-capable significand
#[repr(C)]
pub struct Decimal {
    significand: MagnitudePair,
    exponent: i64,
    precision_bits: u32,
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<Decimal>() == 40);
    assert!(core::mem::align_of::<Decimal>() == 8);
};

impl core::fmt::Debug for Decimal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Decimal")
            .field("sign", &self.sign())
            .field("significand", &self.significand())
            .field("exponent", &self.exponent)
            .field("precision_bits", &self.precision_bits)
            .finish()
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.sign() == other.sign()
            && self.significand.as_limbs() == other.significand.as_limbs()
            && self.exponent == other.exponent
            && self.precision_bits == other.precision_bits
    }
}

impl Eq for Decimal {}

impl Decimal {
    /// Limb1 / Limb2 significand 栈拷贝；Heap 返回 `None`。
    pub fn clone_inline(&self) -> Option<Self> {
        Some(Self::from_parts(self.significand.clone_inline()?, self.exponent, self.precision_bits))
    }

    /// Owning 深复制（Living `19`）。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        Ok(Self::from_parts(self.significand.try_clone().map_err(gc_alloc_error)?, self.exponent, self.precision_bits))
    }

    fn from_parts(significand: MagnitudePair, exponent: i64, precision_bits: u32) -> Self {
        Self { significand, exponent, precision_bits }
    }

    fn from_dyadic_parts(dyadic: Dyadic, precision_bits: u32) -> Self {
        // 从语义 API 视图拆回扁平字段（`dyadic()` 的逆）。
        let sign = dyadic.sign();
        let mut significand = dyadic.significand().into_pair();
        match sign {
            Sign::Negative => significand.set_sign_bit(true),
            Sign::Positive | Sign::Zero => significand.set_sign_bit(false),
        }
        Self::from_parts(significand, dyadic.exponent(), precision_bits)
    }

    /// 最小工作精度下的规范零（`+0`）。
    ///
    /// 须保留调用方工作精度时优先用 [`Self::zero_with_precision`]。
    pub fn zero() -> Self {
        Self::from_parts(MagnitudePair::zero(), 0, MIN_PRECISION_BITS)
    }

    /// 指定工作精度（`≥ 1`）下的规范零（`+0`）。
    pub fn zero_with_precision(precision_bits: u32) -> Result<Self> {
        if precision_bits < MIN_PRECISION_BITS {
            return Err(invalid("precision_zero"));
        }
        Ok(Self::from_parts(MagnitudePair::zero(), 0, precision_bits))
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
        Ok(Self::from_dyadic_parts(dyadic, precision_bits))
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

    /// 精确二元有理载荷视图（按需组装，非存储字段）。
    pub fn dyadic(&self) -> Dyadic {
        Dyadic::try_new(self.sign(), self.significand(), self.exponent).expect("Decimal invariant implies valid Dyadic")
    }

    /// 符号。
    pub fn sign(&self) -> Sign {
        if self.significand.is_zero() {
            if self.significand.sign_bit() { Sign::Negative } else { Sign::Zero }
        }
        else if self.significand.is_negative() {
            Sign::Negative
        }
        else {
            Sign::Positive
        }
    }

    /// 无符号尾数幅度（可失败 owning 复制）。
    pub fn try_significand(&self) -> athena_gc::Result<Natural> {
        Ok(Natural::from_pair(self.significand.try_clone_clear_sign()?))
    }

    /// 无符号尾数幅度（与 [`crate::Integer::abs`] 同合同的便利入口）。
    pub fn significand(&self) -> Natural {
        self.try_significand().expect("portable default max_limbs unbounded")
    }

    /// 二进制指数。
    pub fn exponent(&self) -> i64 {
        self.exponent
    }

    /// 声明的工作精度（位）。
    pub fn precision_bits(&self) -> u32 {
        self.precision_bits
    }

    /// 是否恰为零。
    pub fn is_zero(&self) -> bool {
        self.significand.is_zero()
    }

    /// 是否恰为 `+1`。
    pub fn is_one(&self) -> bool {
        self.sign() == Sign::Positive && self.significand.as_limbs() == [1] && self.exponent == 0
    }

    /// 去掉精确载荷末尾的二进制零。
    pub fn normalize(&mut self) -> Result<()> {
        let mut d = self.dyadic();
        d.normalize();
        *self = Self::from_dyadic_parts(d, self.precision_bits);
        self.validate()
    }

    /// 校验规范不变量与精度合同。
    pub fn validate(&self) -> Result<()> {
        if self.precision_bits < MIN_PRECISION_BITS {
            return Err(invalid("precision_zero"));
        }
        self.dyadic().validate()?;
        if !self.is_zero() && self.significand_bits() > u64::from(self.precision_bits) {
            return Err(precision_exceeds(self.significand_bits(), self.precision_bits));
        }
        Ok(())
    }

    fn significand_bits(&self) -> u64 {
        Natural::bits_from_limbs(self.significand.as_limbs())
    }

    /// 载荷可精确表示时导出为 `f64`。
    pub fn to_f64_exact(&self) -> Option<f64> {
        self.dyadic().to_f64_exact()
    }

    /// 舍入到最近 IEEE binary64（平局取偶）。
    pub fn to_f64_round_nearest_even(&self) -> Option<f64> {
        self.dyadic().to_f64_round_nearest_even()
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
        if self.is_zero() {
            return Ok((Self::from_parts(MagnitudePair::zero(), 0, precision_bits), RoundingStatus::Exact));
        }
        let bits = self.significand_bits();
        if bits <= u64::from(precision_bits) {
            return Ok((
                Self::from_parts(self.significand.try_clone().map_err(gc_alloc_error)?, self.exponent, precision_bits),
                RoundingStatus::Exact,
            ));
        }

        let discard = bits - u64::from(precision_bits);
        let sig = self.significand();
        let mut truncated = sig.shr_bits(discard);
        let round_bit = discard > 0 && sig.bit(discard - 1);
        let sticky = discard > 1 && sig.any_bits_below(discard - 1);
        let lsb = truncated.bit(0);
        let positive = self.sign() != Sign::Negative;

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
                let exp =
                    self.exponent.checked_add(discard as i64).and_then(|e| e.checked_add(1)).ok_or_else(|| invalid("exponent_overflow"))?;
                let dyadic = Dyadic::try_new(self.sign(), truncated, exp)?;
                let value = Self::try_from_dyadic(dyadic, precision_bits)?;
                return Ok((value, status));
            }
        }

        if matches!(mode, RoundingPolicy::TowardPosInf | RoundingPolicy::TowardNegInf) && !positive && (round_bit || sticky) {
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

        let exp = self.exponent.checked_add(discard as i64).ok_or_else(|| invalid("exponent_overflow"))?;
        let dyadic = Dyadic::try_new(self.sign(), truncated, exp)?;
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
