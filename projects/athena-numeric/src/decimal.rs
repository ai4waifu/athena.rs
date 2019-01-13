//! Binary arbitrary-precision real with explicit working precision.
//!
//! [`Decimal`] stores a **rounded** float payload: significand width must not exceed
//! [`Self::precision_bits`]. For exact dyadic values without a precision contract use [`Dyadic`].
//!
//! Rounding is performed on [`Natural`] limbs (guard / round / sticky). It must **not** bridge
//! through IEEE binary64.

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{dyadic::Dyadic, integer::Sign, natural::Natural, rounding::RoundingPolicy};

/// Minimum allowed working precision (at least one significand bit).
pub const MIN_PRECISION_BITS: u32 = 1;

/// IEEE binary64 significand width including implicit bit.
pub const IEEE754_BINARY64_PRECISION: u32 = 53;

/// Status of a rounding operation on a [`Decimal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoundingStatus {
    /// No information discarded; value unchanged relative to the infinite-precision payload.
    Exact,
    /// Magnitude increased by rounding (toward ±∞ away from zero in absolute value).
    RoundedUp,
    /// Magnitude decreased by rounding (toward zero in absolute value).
    RoundedDown,
    /// Information was discarded but direction is not classified (reserved / directed ties).
    Inexact,
}

/// Finite-precision binary float (`Dyadic` payload + declared precision).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decimal {
    dyadic: Dyadic,
    precision_bits: u32,
}

impl Decimal {
    /// Canonical zero (`+0`) at minimum working precision.
    ///
    /// Prefer [`Self::zero_with_precision`] when a caller-owned working precision must be preserved.
    pub fn zero() -> Self {
        Self { dyadic: Dyadic::zero(), precision_bits: MIN_PRECISION_BITS }
    }

    /// Canonical zero (`+0`) at the requested working precision (`>= 1`).
    pub fn zero_with_precision(precision_bits: u32) -> Result<Self> {
        if precision_bits < MIN_PRECISION_BITS {
            return Err(invalid("precision_zero"));
        }
        Ok(Self { dyadic: Dyadic::zero(), precision_bits })
    }

    /// Wrap an exact [`Dyadic`] when its significand fits `precision_bits`.
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

    /// Construct from raw parts (exact dyadic must fit precision).
    pub fn try_new(sign: Sign, significand: Natural, exponent: i64, precision_bits: u32) -> Result<Self> {
        let dyadic = Dyadic::try_new(sign, significand, exponent)?;
        Self::try_from_dyadic(dyadic, precision_bits)
    }

    /// Import finite `f64` with honest 53-bit working precision.
    pub fn from_f64(x: f64) -> Result<Self> {
        let dyadic = Dyadic::from_f64(x)?;
        Self::try_from_dyadic(dyadic, IEEE754_BINARY64_PRECISION)
    }

    /// Exact dyadic payload.
    pub fn dyadic(&self) -> &Dyadic {
        &self.dyadic
    }

    /// Sign.
    pub fn sign(&self) -> Sign {
        self.dyadic.sign()
    }

    /// Unsigned significand magnitude.
    pub fn significand(&self) -> &Natural {
        self.dyadic.significand()
    }

    /// Binary exponent.
    pub fn exponent(&self) -> i64 {
        self.dyadic.exponent()
    }

    /// Declared working precision in bits.
    pub fn precision_bits(&self) -> u32 {
        self.precision_bits
    }

    /// Whether exactly zero.
    pub fn is_zero(&self) -> bool {
        self.dyadic.is_zero()
    }

    /// Whether exactly one (`+1`).
    pub fn is_one(&self) -> bool {
        self.dyadic.is_one()
    }

    /// Strip trailing binary zeros on the exact payload.
    pub fn normalize(&mut self) {
        self.dyadic.normalize();
        if let Err(e) = self.validate() {
            panic!("BigFloat invariant broken after normalize: {:?}", e);
        }
    }

    /// Check canonical invariants and precision contract.
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

    /// Export to `f64` when the payload is exactly representable.
    pub fn to_f64_exact(&self) -> Option<f64> {
        self.dyadic.to_f64_exact()
    }

    /// Round to nearest IEEE binary64 (ties to even).
    pub fn to_f64_round_nearest_even(&self) -> Option<f64> {
        self.dyadic.to_f64_round_nearest_even()
    }

    /// Lossy `f64` after rounding to nearest even.
    pub fn to_f64_approximate(&self) -> Option<f64> {
        self.to_f64_round_nearest_even()
    }

    /// Round payload to a new working precision (nearest even, limb kernel).
    pub fn round_to_precision(&self, precision_bits: u32) -> Result<(Self, RoundingStatus)> {
        self.round_to_precision_with_mode(precision_bits, RoundingPolicy::NearestEven)
    }

    /// Round payload to a new working precision under an explicit rounding policy.
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
            // Carry out of the p-bit window: 1 << precision_bits.
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

        // Toward ±∞ on a negative value: increasing magnitude is RoundedUp in abs sense already.
        // For signed directed modes, remapped status when we truncated toward +∞ on a negative:
        if matches!(mode, RoundingPolicy::TowardPosInf | RoundingPolicy::TowardNegInf) && !positive && (round_bit || sticky) {
            // Negative + TowardPosInf truncates toward zero → magnitude down → RoundedDown
            // Negative + TowardNegInf rounds away from zero → magnitude up → RoundedUp
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
