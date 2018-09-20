//! Real numbers (machine / arbitrary-precision skeleton).
//!
//! Precision and rounding context live only on [`crate::number::NumericValue::precision`],
//! not duplicated inside [`Real`].

/// Real value representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Real {
    /// IEEE binary64.
    Machine(f64),
    /// Arbitrary-precision skeleton: exact IEEE754 bit pattern + working significand bits.
    ///
    /// Not a decimal execution payload; a future backend may replace this with limbs.
    /// Working precision is owned by the outer [`crate::number::NumericValue`] via
    /// [`crate::precision::PrecisionInfo`]; `working_bits` must match (enforced by constructors).
    Arbitrary {
        /// IEEE754 binary64 bit pattern (exact round-trip for machine promotion).
        ieee754_bits: u64,
        /// Working significand precision in bits.
        working_bits: u32,
    },
    /// Placeholder when a backend is not yet available; not usable in arithmetic.
    Unsupported,
}

impl Real {
    /// Machine real.
    pub fn machine(x: f64) -> Self {
        Self::Machine(x)
    }

    /// Promote a machine value into the arbitrary skeleton (preserves IEEE754 bits).
    pub(crate) fn from_machine_promoted(x: f64, working_bits: u32) -> Self {
        Self::Arbitrary { ieee754_bits: x.to_bits(), working_bits }
    }

    /// Working precision in bits for arbitrary skeleton values.
    pub fn working_bits(&self) -> Option<u32> {
        match self {
            Self::Machine(_) => None,
            Self::Arbitrary { working_bits, .. } => Some(*working_bits),
            Self::Unsupported => None,
        }
    }

    /// Machine `f64` view (direct for Machine; via IEEE754 bits for Arbitrary).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Machine(x) => Some(*x),
            Self::Arbitrary { ieee754_bits, .. } => Some(f64::from_bits(*ieee754_bits)),
            Self::Unsupported => None,
        }
    }

    /// Whether the value is finite.
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Machine(x) => x.is_finite(),
            Self::Arbitrary { ieee754_bits, .. } => f64::from_bits(*ieee754_bits).is_finite(),
            Self::Unsupported => false,
        }
    }
}

impl Default for Real {
    fn default() -> Self {
        Self::Machine(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Real;

    #[test]
    fn machine_promoted_roundtrip_preserves_bits() {
        let x = 1.5_f64;
        let arb = Real::from_machine_promoted(x, 53);
        assert_eq!(arb.as_f64(), Some(x));
        match arb {
            Real::Arbitrary { ieee754_bits, working_bits } => {
                assert_eq!(ieee754_bits, x.to_bits());
                assert_eq!(working_bits, 53);
            }
            _ => panic!("expected arbitrary"),
        }
    }

    #[test]
    fn arbitrary_is_finite_matches_ieee754() {
        let nan = Real::from_machine_promoted(f64::NAN, 53);
        assert!(!nan.is_finite());
        let inf = Real::from_machine_promoted(f64::INFINITY, 53);
        assert!(!inf.is_finite());
        let fin = Real::from_machine_promoted(1.0, 53);
        assert!(fin.is_finite());
    }
}
