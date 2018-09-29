//! Numeric backend contract (Living `13` / `16`).

use crate::{domain::NumericDomain, precision::PrecisionKind};

/// Capability flags for dispatch and host reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericCapability {
    /// Exact integer arithmetic.
    ExactInteger,
    /// Exact rational arithmetic.
    ExactRational,
    /// IEEE binary64 machine reals.
    MachineReal,
    /// Arbitrary-real skeleton (IEEE754 promotion path).
    ArbitraryRealSkeleton,
    /// Modular integer reduction.
    ModularInteger,
    /// Interval enclosure skeleton.
    IntervalEnclosure,
    /// Directed rounding for interval endpoints.
    DirectedRounding,
    /// Promotion with explicit diagnostics.
    ExplicitPromotion,
    /// Deterministic pure-Rust execution.
    Deterministic,
}

/// Numeric operations backends may advertise and dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericOperation {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Exponentiation.
    Pow,
    /// Negation.
    Neg,
    /// Absolute value.
    Abs,
    /// Square root.
    Sqrt,
    /// Factorial.
    Factorial,
    /// Greatest common divisor.
    Gcd,
    /// Ordered comparison.
    Compare,
    /// Domain / precision promotion.
    Promote,
    /// Interval addition.
    IntervalAdd,
    /// Interval multiplication.
    IntervalMul,
}

/// Result semantics a backend guarantees for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericResultMode {
    /// Exact symbolic / integer / rational result.
    Exact,
    /// IEEE binary64 machine result.
    Machine,
    /// Arbitrary-real skeleton (IEEE754 bit pattern).
    ArbitrarySkeleton,
    /// Interval enclosure with directed rounding.
    IntervalEnclosure,
    /// Certified result with attached proof metadata.
    Certified,
}

/// Resource and wire limits advertised by a backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericBackendLimits {
    /// Maximum limb count for big integers (None = unbounded contract).
    pub max_limbs: Option<u32>,
    /// Maximum significand bits for arbitrary reals.
    pub max_significand_bits: Option<u32>,
    /// Maximum UTF-8 bytes for canonical wire integer/rational payload decode.
    pub max_wire_payload_bytes: Option<u32>,
    /// Maximum exponent magnitude for `pow` (None = backend default policy).
    pub max_pow_exp: Option<i64>,
}

impl Default for NumericBackendLimits {
    fn default() -> Self {
        Self {
            max_limbs: None,
            max_significand_bits: Some(53),
            max_wire_payload_bytes: Some(1 << 20),
            max_pow_exp: Some(10_000),
        }
    }
}

/// Static backend contract (capabilities, limits, availability).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericBackendContract {
    /// Stable backend id.
    pub id: &'static str,
    /// Safe for wasm32 builds.
    pub wasm_safe: bool,
    /// Requires native-only libraries (BLAS/MKL/MPFR candidates).
    pub native_only: bool,
    /// May participate in JIT compilation plans.
    pub jit_eligible: bool,
    /// Reproducible results under fixed policy.
    pub deterministic: bool,
    /// Canonical textual wire radix (10 for decimal payloads).
    pub default_radix: u8,
    /// Advertised capabilities.
    pub capabilities: &'static [NumericCapability],
    /// Resource limits.
    pub limits: NumericBackendLimits,
}

/// Numeric backend capability surface.
pub trait NumericBackend {
    /// Full static contract.
    fn contract(&self) -> &'static NumericBackendContract;

    /// Backend name.
    fn name(&self) -> &'static str {
        self.contract().id
    }

    /// Whether the backend is usable on wasm32.
    fn wasm_safe(&self) -> bool {
        self.contract().wasm_safe
    }

    /// Whether a capability is advertised.
    fn has_capability(&self, cap: NumericCapability) -> bool {
        self.contract().capabilities.contains(&cap)
    }

    /// Whether the backend supports a domain in the current maturity gate.
    fn supports_domain(&self, domain: &NumericDomain) -> bool;

    /// Whether the backend supports a precision kind.
    fn supports_precision(&self, kind: PrecisionKind) -> bool;

    /// Whether the backend can perform `op` on `domain` producing `result`.
    fn supports_operation(&self, domain: &NumericDomain, op: NumericOperation, result: NumericResultMode) -> bool;
}

/// Default pure-Rust backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct PureRustBackend;

const PURE_RUST_WIRE_PAYLOAD_LIMIT: u32 = 1 << 20;

const PURE_RUST_CAPS: &[NumericCapability] = &[
    NumericCapability::ExactInteger,
    NumericCapability::ExactRational,
    NumericCapability::MachineReal,
    NumericCapability::ArbitraryRealSkeleton,
    NumericCapability::ModularInteger,
    NumericCapability::IntervalEnclosure,
    NumericCapability::DirectedRounding,
    NumericCapability::ExplicitPromotion,
    NumericCapability::Deterministic,
];

const PURE_RUST_CONTRACT: NumericBackendContract = NumericBackendContract {
    id: "pure-rust",
    wasm_safe: true,
    native_only: false,
    jit_eligible: true,
    deterministic: true,
    default_radix: 10,
    capabilities: PURE_RUST_CAPS,
    limits: NumericBackendLimits {
        max_limbs: None,
        max_significand_bits: Some(53),
        max_wire_payload_bytes: Some(PURE_RUST_WIRE_PAYLOAD_LIMIT),
        max_pow_exp: Some(crate::integer::Integer::MAX_POW_EXP),
    },
};

/// Wire payload byte limit for the pure-Rust backend (shared with decode).
pub(crate) const PURE_RUST_WIRE_PAYLOAD_LIMIT_BYTES: u32 = PURE_RUST_WIRE_PAYLOAD_LIMIT;

impl NumericBackend for PureRustBackend {
    fn contract(&self) -> &'static NumericBackendContract {
        &PURE_RUST_CONTRACT
    }

    fn supports_domain(&self, domain: &NumericDomain) -> bool {
        matches!(
            domain,
            NumericDomain::Integer
                | NumericDomain::Rational
                | NumericDomain::Real
                | NumericDomain::Complex
                | NumericDomain::Interval
                | NumericDomain::Modular { .. }
        )
    }

    fn supports_precision(&self, kind: PrecisionKind) -> bool {
        matches!(kind, PrecisionKind::Exact | PrecisionKind::Machine | PrecisionKind::Arbitrary | PrecisionKind::Interval)
    }

    fn supports_operation(&self, domain: &NumericDomain, op: NumericOperation, result: NumericResultMode) -> bool {
        if !self.supports_domain(domain) {
            return false;
        }
        match (domain, op, result) {
            (
                NumericDomain::Integer,
                NumericOperation::Add
                | NumericOperation::Sub
                | NumericOperation::Mul
                | NumericOperation::Div
                | NumericOperation::Pow
                | NumericOperation::Neg
                | NumericOperation::Abs
                | NumericOperation::Gcd
                | NumericOperation::Compare
                | NumericOperation::Factorial,
                NumericResultMode::Exact,
            ) => true,
            (
                NumericDomain::Rational,
                NumericOperation::Add
                | NumericOperation::Sub
                | NumericOperation::Mul
                | NumericOperation::Div
                | NumericOperation::Pow
                | NumericOperation::Neg
                | NumericOperation::Abs
                | NumericOperation::Compare,
                NumericResultMode::Exact,
            ) => true,
            (NumericDomain::Integer | NumericDomain::Rational, NumericOperation::Promote, NumericResultMode::Machine) => {
                self.has_capability(NumericCapability::ExplicitPromotion)
            }
            (
                NumericDomain::Real,
                NumericOperation::Add
                | NumericOperation::Sub
                | NumericOperation::Mul
                | NumericOperation::Div
                | NumericOperation::Compare
                | NumericOperation::Promote,
                NumericResultMode::Machine,
            ) => true,
            (NumericDomain::Real, NumericOperation::Promote, NumericResultMode::ArbitrarySkeleton) => {
                self.has_capability(NumericCapability::ArbitraryRealSkeleton)
            }
            (
                NumericDomain::Interval,
                NumericOperation::IntervalAdd | NumericOperation::IntervalMul,
                NumericResultMode::IntervalEnclosure,
            ) => {
                self.has_capability(NumericCapability::IntervalEnclosure)
                    && self.has_capability(NumericCapability::DirectedRounding)
            }
            (
                NumericDomain::Modular { .. },
                NumericOperation::Add | NumericOperation::Sub | NumericOperation::Mul | NumericOperation::Pow,
                NumericResultMode::Exact,
            ) => self.has_capability(NumericCapability::ModularInteger),
            _ => false,
        }
    }
}
