//! Closed mathematical constants (Living `27` frontend-neutral atoms).
//!
//! Dialect surface names (`Pi`, `pi`, `π`, `E`, `e`, `ℯ`) map here only in SXO lowering.
//! Athena execution must never re-infer these from user symbol display names.

/// Typed mathematical constant atom payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MathematicalConstant {
    /// Circle constant $\pi$.
    Pi,
    /// Base of the natural logarithm $e$.
    EulerNumber,
}

impl MathematicalConstant {
    /// Stable discriminant for fingerprints / wire.
    pub const fn discriminant(self) -> u8 {
        match self {
            Self::Pi => 1,
            Self::EulerNumber => 2,
        }
    }

    /// Debug / diagnostics label (not a dialect surface name contract).
    pub const fn debug_label(self) -> &'static str {
        match self {
            Self::Pi => "Pi",
            Self::EulerNumber => "E",
        }
    }
}
