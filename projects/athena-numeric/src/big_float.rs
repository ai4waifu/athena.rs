//! Binary arbitrary-precision real: `sign · significand · 2^exponent`.
//!
//! Invariants (finite, after [`Self::normalize`]):
//! - Zero → `sign == Zero`, `significand == 0`, `exponent == 0`
//! - Non-zero → `significand` is odd (no trailing binary zeros)
//! - `precision_bits >= 1` and records working significand width (not hidden payload)

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{integer::Sign, natural::Natural};

/// Minimum allowed working precision (implicit bit + at least one fraction bit).
pub const MIN_PRECISION_BITS: u32 = 2;

/// IEEE binary64 significand width including implicit bit.
pub const IEEE754_BINARY64_PRECISION: u32 = 53;

/// Binary float with explicit working precision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigFloat {
    sign: Sign,
    significand: Natural,
    exponent: i64,
    precision_bits: u32,
}

impl BigFloat {
    /// Canonical zero (`+0`).
    pub fn zero() -> Self {
        Self { sign: Sign::Zero, significand: Natural::zero(), exponent: 0, precision_bits: 1 }
    }

    /// Construct and normalize; rejects non-positive `precision_bits`.
    pub fn try_new(sign: Sign, significand: Natural, exponent: i64, precision_bits: u32) -> Result<Self> {
        if precision_bits == 0 {
            return Err(invalid("precision_zero"));
        }
        let mut v = Self { sign, significand, exponent, precision_bits };
        v.normalize();
        v.validate()?;
        Ok(v)
    }

    /// Import a finite IEEE binary64 value with honest 53-bit working precision.
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
                precision_bits: 1,
            });
        }

        let (sign, significand, exponent) = if exp_field == 0 {
            (
                if negative { Sign::Negative } else { Sign::Positive },
                Natural::from_u64(frac),
                1i64 - 1023 - 52,
            )
        }
        else {
            (
                if negative { Sign::Negative } else { Sign::Positive },
                Natural::from_u64(frac | (1u64 << 52)),
                i64::from(exp_field) - 1023 - 52,
            )
        };

        let mut v = Self { sign, significand, exponent, precision_bits: IEEE754_BINARY64_PRECISION };
        v.normalize();
        debug_assert!(v.validate().is_ok());
        Ok(v)
    }

    /// Sign.
    pub fn sign(&self) -> Sign {
        self.sign
    }

    /// Unsigned significand magnitude.
    pub fn significand(&self) -> &Natural {
        &self.significand
    }

    /// Binary exponent (`value = sign · significand · 2^exponent`).
    pub fn exponent(&self) -> i64 {
        self.exponent
    }

    /// Declared working precision in bits.
    pub fn precision_bits(&self) -> u32 {
        self.precision_bits
    }

    /// Whether the value is exactly zero.
    pub fn is_zero(&self) -> bool {
        self.significand.is_zero()
    }

    /// Whether the value is exactly one (`+1`).
    pub fn is_one(&self) -> bool {
        self.sign == Sign::Positive && self.significand.is_one() && self.exponent == 0
    }

    /// Strip trailing binary zeros and canonicalize zero sign.
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

    /// Check canonical invariants.
    pub fn validate(&self) -> Result<()> {
        if self.precision_bits == 0 {
            return Err(invalid("precision_zero"));
        }
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

    /// Export to `f64` when the value is exactly representable (no rounding).
    pub fn to_f64_exact(&self) -> Option<f64> {
        if self.significand.is_zero() {
            return Some(if self.sign == Sign::Negative { -0.0 } else { 0.0 });
        }
        ieee::encode_finite(self.sign, &self.significand, self.exponent)
    }

    /// Round to nearest IEEE binary64 (ties to even). Used for controlled promotion to machine.
    pub fn to_f64_round_nearest_even(&self) -> Option<f64> {
        if self.significand.is_zero() {
            return Some(if self.sign == Sign::Negative { -0.0 } else { 0.0 });
        }
        ieee::round_to_binary64(self.sign, &self.significand, self.exponent)
    }

    /// Lossy `f64` when magnitude fits the IEEE range after rounding.
    pub fn to_f64_approximate(&self) -> Option<f64> {
        self.to_f64_round_nearest_even()
    }
}

mod ieee {
    use super::{Natural, Sign};

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
        if sig.bits() <= 53 {
            return encode_finite(sign, sig, exp);
        }
        let shift = sig.bits() - 53;
        if shift > 63 {
            return None;
        }
        let mut mant = sig.to_u64()? >> shift;
        let mut exp = exp + i64::from(shift as u32);
        let rem_mask = (1u64 << shift) - 1;
        let rem = sig.to_u64()? & rem_mask;
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

        // Subnormal: value = mant · 2^exp = frac · 2^-1074
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_exact_roundtrip_normals_and_subnormals() {
        let samples = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.5,
            3.0,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::MIN,
            1.5,
            f64::from_bits(0x0000_0000_0000_0001),
        ];
        for x in samples {
            let bf = BigFloat::from_f64(x).expect("finite");
            bf.validate().expect("valid");
            let back = bf.to_f64_exact().expect("exact");
            assert_eq!(back.to_bits(), x.to_bits(), "failed for {x:?}");
        }
    }

    #[test]
    fn normalize_strips_trailing_zeros() {
        let bf = BigFloat::try_new(Sign::Positive, Natural::from_u64(12), 0, 4).unwrap();
        assert_eq!(bf.significand().to_u64(), Some(3));
        assert_eq!(bf.exponent(), 2);
    }

    #[test]
    fn rejects_nan_import() {
        assert!(BigFloat::from_f64(f64::NAN).is_err());
    }
}
