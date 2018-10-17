//! Numeric backend contract and dispatch (Living `13` / `16`).
//!
//! Layout:
//! ```text
//! backends/
//!   mod.rs           — trait, capabilities, limits
//!   pure-rust/       — default WASM-safe kernel (limb arithmetic + `Natural`)
//! ```
//!
//! Future optional backends (e.g. `native-accelerated/`) live as sibling directories;
//! the Rust module name uses underscores (`pure_rust`) because identifiers cannot contain `-`.

#[path = "pure-rust/mod.rs"]
pub mod pure_rust;

pub use pure_rust::PureRustBackend;

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

/// Wire payload byte limit for the default pure-Rust backend (shared with decode).
pub(crate) use pure_rust::PURE_RUST_WIRE_PAYLOAD_LIMIT_BYTES;
