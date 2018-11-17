//! Athena 数值塔 — 表示、运算、精度、promotion、证书（Living `16` N0–N2）。
#![deny(missing_docs)]

pub mod algebraic;
pub mod backends;
pub mod big_float;
pub mod certificate;
pub mod comparison;
pub mod complex;
pub mod domain;
pub mod dyadic;
pub mod evidence;
pub mod execution_budget;
pub mod finite_field;
pub mod integer;
pub mod interval;
pub mod kernel_number;
pub mod modular;
pub mod natural;
pub mod number;
pub mod number_wire;
pub mod p_adic;
pub mod polynomial_fingerprint;
pub mod precision;
pub mod promotion;
pub mod rational;
pub mod real;
pub mod rounding;
pub mod serialization;
pub mod wire_binary;
pub mod wire_text;

pub use algebraic::AlgebraicNumber;
pub use backends::{
    NumericBackend, NumericBackendContract, NumericBackendLimits, NumericCapability, NumericOperation, NumericResultMode,
    PureRustBackend,
};
pub use big_float::BigFloat;
pub use certificate::{CertificateMethod, NumericCertificate};
pub use comparison::{ComparisonPolicy, DefaultNumericCompare, NumericCompare, NumericComparison};
pub use complex::{BranchPolicy, Complex};
pub use domain::NumericDomain;
pub use dyadic::Dyadic;
#[allow(deprecated)]
pub use evidence::NumericProvenance;
pub use evidence::{NumericBinding, NumericEvidenceArena, NumericEvidenceId, NumericEvidenceRecord};
pub use execution_budget::{ExecutionBudget, NumericContext};
pub use finite_field::FiniteFieldValue;
pub use integer::{ExactInteger, Integer, Sign};
pub use interval::{Interval, IntervalDecoration};
pub use kernel_number::{abs, add, compare, div, factorial, mul, neg, pow, sqrt, to_f64_lossy};
pub use modular::{ModularValue, Modulus};
pub use number::{Number, NumericValue};
pub use number_wire::{from_wire as number_from_wire, to_wire as number_to_wire};
pub use p_adic::PAdicValue;
pub use polynomial_fingerprint::PolynomialFingerprint;
pub use precision::{PrecisionInfo, PrecisionKind};
pub use promotion::{DefaultPromotion, Promotion, PromotionPolicy};
pub use rational::{ExactRational, Rational};
pub use real::Real;
pub use rounding::RoundingPolicy;
pub use serialization::NumericValueWire;
