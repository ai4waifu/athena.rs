//! Binary arbitrary-precision real with explicit working precision.
//!
//! [`BigFloat`] stores a **rounded** float payload: significand width must not exceed
//! [`Self::precision_bits`]. For exact dyadic values without a precision contract use [`Dyadic`].

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{dyadic::Dyadic, integer::Sign, natural::Natural};

/// Minimum allowed working precision (implicit bit + at least one fraction bit).
pub const MIN_PRECISION_BITS: u32 = 2;

/// IEEE binary64 significand width including implicit bit.
pub const IEEE754_BINARY64_PRECISION: u32 = 53;

/// Finite-precision binary float (`Dyadic` payload + declared precision).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigFloat {
    dyadic: Dyadic,
    precision_bits: u32,
}

impl BigFloat {
    /// Canonical zero (`+0`) at 1-bit working precision.
    pub fn zero() -> Self {
        Self { dyadic: Dyadic::zero(), precision_bits: 1 }
    }

    /// Wrap an exact [`Dyadic`] when its significand fits `precision_bits`.
    pub fn try_from_dyadic(dyadic: Dyadic, precision_bits: u32) -> Result<Self> {
        if precision_bits == 0 {
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
        if self.precision_bits == 0 {
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

    /// Round payload to a new working precision (nearest even via `f64` bridge).
    pub fn round_to_precision(&self, precision_bits: u32) -> Result<Self> {
        if precision_bits == 0 {
            return Err(invalid("precision_zero"));
        }
        if self.dyadic.is_zero() {
            return Ok(Self { dyadic: Dyadic::zero(), precision_bits });
        }
        if self.dyadic.significand_bits() <= u64::from(precision_bits) {
            return Ok(Self { dyadic: self.dyadic.clone(), precision_bits });
        }
        let approx = self.to_f64_round_nearest_even().ok_or_else(|| invalid("round_failed"))?;
        let dyadic = Dyadic::from_f64(approx)?;
        Self::try_from_dyadic(dyadic, precision_bits)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_exact_roundtrip_normals_and_subnormals() {
        let samples =
            [0.0, -0.0, 1.0, -1.0, 0.5, 3.0, f64::MIN_POSITIVE, f64::MAX, f64::MIN, 1.5, f64::from_bits(0x0000_0000_0000_0001)];
        for x in samples {
            let bf = BigFloat::from_f64(x).expect("finite");
            bf.validate().expect("valid");
            let back = bf.to_f64_exact().expect("exact");
            assert_eq!(back.to_bits(), x.to_bits(), "failed for {x:?}");
        }
    }

    #[test]
    fn rejects_payload_wider_than_precision() {
        let sig = Natural::from_limbs(vec![9007199254740993, 1]);
        let err = BigFloat::try_new(Sign::Positive, sig, 0, 53).unwrap_err();
        assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_PRECISION_LOSS");
    }

    #[test]
    fn rejects_nan_import() {
        assert!(BigFloat::from_f64(f64::NAN).is_err());
    }
}
