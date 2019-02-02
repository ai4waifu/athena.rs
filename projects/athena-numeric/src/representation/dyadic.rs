//! 精确二元有理：`sign × significand × 2^exponent`，非零时尾数规范为奇数。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{integer::Sign, natural::Natural};

/// 精确二进制有理：`sign · significand · 2^exponent`（非零时尾数为奇）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dyadic {
    sign: Sign,
    significand: Natural,
    exponent: i64,
}

impl Dyadic {
    /// 规范零（`+0`）。
    pub fn zero() -> Self {
        Self { sign: Sign::Zero, significand: Natural::zero(), exponent: 0 }
    }

    /// 构造并规范化；规范化后拒绝非法形状。
    pub fn try_new(sign: Sign, significand: Natural, exponent: i64) -> Result<Self> {
        let mut v = Self { sign, significand, exponent };
        v.normalize();
        v.validate()?;
        Ok(v)
    }

    /// 将有限 IEEE binary64 导入为精确二元有理。
    pub fn from_f64(x: f64) -> Result<Self> {
        if x.is_nan() || x.is_infinite() {
            return Err(invalid("non_finite_f64"));
        }
        let bits = x.to_bits();
        let negative = bits >> 63 != 0;
        let exp_field = ((bits >> 52) & 0x7ff) as u32;
        let frac = bits & 0x000f_ffff_ffff_ffff;

        if exp_field == 0 && frac == 0 {
            return Ok(Self {
                sign: if negative { Sign::Negative } else { Sign::Zero },
                significand: Natural::zero(),
                exponent: 0,
            });
        }

        let (sign, significand, exponent) = if exp_field == 0 {
            (if negative { Sign::Negative } else { Sign::Positive }, Natural::from_u64(frac), 1i64 - 1023 - 52)
        }
        else {
            (
                if negative { Sign::Negative } else { Sign::Positive },
                Natural::from_u64(frac | (1u64 << 52)),
                i64::from(exp_field) - 1023 - 52,
            )
        };

        let mut v = Self { sign, significand, exponent };
        v.normalize();
        debug_assert!(v.validate().is_ok());
        Ok(v)
    }

    /// 符号。
    pub fn sign(&self) -> Sign {
        self.sign
    }

    /// 无符号尾数幅度。
    pub fn significand(&self) -> &Natural {
        &self.significand
    }

    /// 二进制指数。
    pub fn exponent(&self) -> i64 {
        self.exponent
    }

    /// 是否恰为零。
    pub fn is_zero(&self) -> bool {
        self.significand.is_zero()
    }

    /// 是否恰为 `+1`。
    pub fn is_one(&self) -> bool {
        self.sign == Sign::Positive && self.significand.is_one() && self.exponent == 0
    }

    /// 尾数位宽（零 → 0）。
    pub fn significand_bits(&self) -> u64 {
        self.significand.bits()
    }

    /// 去掉末尾二进制零并规范零的符号。
    pub fn normalize(&mut self) {
        if self.significand.is_zero() {
            if self.sign == Sign::Negative {
                self.exponent = 0;
                return;
            }
            self.sign = Sign::Zero;
            self.exponent = 0;
            return;
        }
        while !self.significand.is_odd() {
            self.significand.div2();
            self.exponent += 1;
        }
        if self.sign == Sign::Zero {
            self.sign = Sign::Positive;
        }
    }

    /// 校验规范不变量。
    pub fn validate(&self) -> Result<()> {
        if self.significand.is_zero() {
            if self.exponent != 0 {
                return Err(invalid("zero_shape"));
            }
            if !matches!(self.sign, Sign::Zero | Sign::Negative) {
                return Err(invalid("zero_sign"));
            }
            return Ok(());
        }
        if self.sign != Sign::Positive && self.sign != Sign::Negative {
            return Err(invalid("nonzero_sign"));
        }
        if !self.significand.is_odd() {
            return Err(invalid("not_normalized"));
        }
        Ok(())
    }

    /// 可精确表示时导出为 `f64`（不舍入）。
    pub fn to_f64_exact(&self) -> Option<f64> {
        ieee::encode_finite(self.sign, &self.significand, self.exponent)
    }

    /// 舍入到最近 IEEE binary64（平局取偶）。
    pub fn to_f64_round_nearest_even(&self) -> Option<f64> {
        ieee::round_to_binary64(self.sign, &self.significand, self.exponent)
    }
}

mod ieee {
    use super::{Natural, Sign};

    use crate::kernel::limb as limb_kernel;

    pub(super) fn encode_finite(sign: Sign, sig: &Natural, exp: i64) -> Option<f64> {
        if sig.is_zero() {
            return Some(if sign == Sign::Negative { -0.0 } else { 0.0 });
        }
        if sig.bits() > 53 {
            return None;
        }
        let mut mant = sig.to_u64()?;
        let mut exp = exp;
        align_mantissa_53(&mut mant, &mut exp);
        pack_finite(sign, mant, exp)
    }

    pub(super) fn round_to_binary64(sign: Sign, sig: &Natural, exp: i64) -> Option<f64> {
        if sig.is_zero() {
            return Some(if sign == Sign::Negative { -0.0 } else { 0.0 });
        }
        let bits = sig.bits();
        if bits <= 53 {
            return encode_finite(sign, sig, exp);
        }
        let shift = bits - 53;
        let (quotient, rem) = limb_kernel::shr_natural(sig.as_limbs(), shift as u32);
        let mut mant = if quotient.len() == 1 && quotient[0] <= u64::MAX {
            quotient[0]
        }
        else {
            return None;
        };
        let mut exp = exp + i64::from(shift as u32);
        let half = 1u64 << (shift - 1);
        if rem > half || (rem == half && (mant & 1) == 1) {
            mant = mant.checked_add(1)?;
            if mant >= 1u64 << 53 {
                mant >>= 1;
                exp += 1;
            }
        }
        align_mantissa_53(&mut mant, &mut exp);
        pack_finite(sign, mant, exp)
    }

    fn align_mantissa_53(mant: &mut u64, exp: &mut i64) {
        while *mant != 0 && *mant < (1u64 << 52) && *exp > -1074 {
            *mant <<= 1;
            *exp -= 1;
        }
        while *mant >= (1u64 << 53) {
            *mant >>= 1;
            *exp += 1;
        }
    }

    fn pack_finite(sign: Sign, mant: u64, exp: i64) -> Option<f64> {
        if mant == 0 {
            return Some(0.0);
        }
        let sign_bit = if sign == Sign::Negative { 1u64 << 63 } else { 0 };

        if mant >= 1u64 << 52 {
            let biased = exp + 1023 + 52;
            if biased >= 0x7ff {
                return None;
            }
            if biased <= 0 {
                return None;
            }
            let frac = mant & ((1u64 << 52) - 1);
            let bits = sign_bit | ((biased as u64) << 52) | frac;
            return Some(f64::from_bits(bits));
        }

        if exp < -1074 {
            return Some(f64::from_bits(sign_bit));
        }
        let shift = exp + 1074;
        if shift < 0 {
            return Some(f64::from_bits(sign_bit));
        }
        if shift >= 52 {
            return None;
        }
        let frac = mant << shift;
        if frac >= 1u64 << 52 {
            return None;
        }
        Some(f64::from_bits(sign_bit | frac))
    }
}

fn invalid(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", op)
}
