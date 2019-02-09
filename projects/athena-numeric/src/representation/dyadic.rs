//! 精确二元有理：`sign × significand × 2^exponent`，非零时尾数规范为奇数。
//!
//! 布局：`significand_meta + Magnitude` + `exponent`（LP64 上 32 bytes）。
//! `Sign` 仅为语义 API，经 `meta` 编解码；禁止嵌套 `Natural` 字段。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{integer::Sign, natural::Natural, storage::MagnitudePair};

/// 精确二进制有理：`sign · significand · 2^exponent`（非零时尾数为奇）。
#[derive(Clone)]
pub struct Dyadic {
    significand: MagnitudePair,
    exponent: i64,
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<Dyadic>() == 32);
    assert!(core::mem::align_of::<Dyadic>() == 8);
};

impl core::fmt::Debug for Dyadic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Dyadic")
            .field("sign", &self.sign())
            .field("significand", &self.significand())
            .field("exponent", &self.exponent)
            .finish()
    }
}

impl PartialEq for Dyadic {
    fn eq(&self, other: &Self) -> bool {
        self.sign() == other.sign()
            && self.significand.as_limbs() == other.significand.as_limbs()
            && self.exponent == other.exponent
    }
}

impl Eq for Dyadic {}

impl Dyadic {
    fn from_parts(significand: MagnitudePair, exponent: i64) -> Self {
        Self { significand, exponent }
    }

    /// 由符号 / 幅度 / 指数组装（不规范化）。
    fn from_sign_mag(sign: Sign, mag: Natural, exponent: i64) -> Self {
        let mut significand = mag.into_pair();
        match sign {
            Sign::Negative => significand.set_sign_bit(true),
            Sign::Positive | Sign::Zero => significand.set_sign_bit(false),
        }
        Self::from_parts(significand, exponent)
    }

    /// 规范零（`+0`）。
    pub fn zero() -> Self {
        Self::from_parts(MagnitudePair::zero(), 0)
    }

    /// 构造并规范化；规范化后拒绝非法形状。
    pub fn try_new(sign: Sign, significand: Natural, exponent: i64) -> Result<Self> {
        let mut v = Self::from_sign_mag(sign, significand, exponent);
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
            let mut v = Self::zero();
            if negative {
                v.significand.set_sign_bit(true);
            }
            return Ok(v);
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

        let mut v = Self::from_sign_mag(sign, significand, exponent);
        v.normalize();
        debug_assert!(v.validate().is_ok());
        Ok(v)
    }

    /// 符号（零可保留 IEEE `-0`：`Sign::Negative`）。
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

    /// 无符号尾数幅度（克隆；不解释 `meta` sign）。
    pub fn significand(&self) -> Natural {
        Natural::from_pair(self.significand.clone_clear_sign())
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
        self.sign() == Sign::Positive && self.significand.as_limbs() == [1] && self.exponent == 0
    }

    /// 尾数位宽（零 → 0）。
    pub fn significand_bits(&self) -> u64 {
        if self.is_zero() {
            0
        }
        else {
            Natural::from_pair(self.significand.clone_clear_sign()).bits()
        }
    }

    /// 去掉末尾二进制零并规范零的符号。
    pub fn normalize(&mut self) {
        if self.significand.is_zero() {
            // 保留 `-0` 的 sign 位；指数归零。
            self.exponent = 0;
            if !self.significand.sign_bit() {
                self.significand = MagnitudePair::zero();
            }
            return;
        }
        let negative = self.significand.is_negative();
        let mut mag = Natural::from_pair(self.significand.clone_clear_sign());
        while !mag.is_odd() {
            mag.div2();
            self.exponent += 1;
        }
        self.significand = mag.into_pair();
        self.significand.set_sign_bit(negative);
    }

    /// 校验规范不变量。
    pub fn validate(&self) -> Result<()> {
        if self.significand.is_zero() {
            if self.exponent != 0 {
                return Err(invalid("zero_shape"));
            }
            // `meta` sign：清零 → `+0`；置位 → IEEE `-0`。
            return Ok(());
        }
        if self.sign() != Sign::Positive && self.sign() != Sign::Negative {
            return Err(invalid("nonzero_sign"));
        }
        let mag = Natural::from_pair(self.significand.clone_clear_sign());
        if !mag.is_odd() {
            return Err(invalid("not_normalized"));
        }
        Ok(())
    }

    /// 可精确表示时导出为 `f64`（不舍入）。
    pub fn to_f64_exact(&self) -> Option<f64> {
        ieee::encode_finite(self.sign(), &self.significand(), self.exponent)
    }

    /// 舍入到最近 IEEE binary64（平局取偶）。
    pub fn to_f64_round_nearest_even(&self) -> Option<f64> {
        ieee::round_to_binary64(self.sign(), &self.significand(), self.exponent)
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
