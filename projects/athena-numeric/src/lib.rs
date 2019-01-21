//! Athena 数值塔 — 表示、运算、精度、promotion、证书。
#![deny(missing_docs)]

pub mod arithmetic;
pub mod backend;
pub mod certificate;
pub mod format;
pub mod kernel;
pub mod policy;
pub mod representation;
pub mod value;

// —— 稳定模块路径（真实模块，不是别名文件）——
pub use value::{
    algebraic, complex, finite_field, integer, interval, modular, natural, number, p_adic, rational, real,
};
pub use representation::{big_float as decimal, domain, dyadic, polynomial_fingerprint, precision};
pub use arithmetic::{comparison, kernel_number, modular_ops as modular_kernel, modulus_context, promotion, rounding};
pub use policy::execution_budget;
pub use format::{binary as wire_binary, serialization, text as wire_text, wire as number_wire};
pub use backend as backends;

pub use value::algebraic::AlgebraicNumber;
pub use backend::{
    NumericBackend, NumericBackendContract, NumericBackendLimits, NumericCapability, NumericOperation, NumericResultMode,
    PureRustBackend,
};
pub use certificate::{CertificateMethod, NumericCertificate};
pub use arithmetic::comparison::{ComparisonPolicy, DefaultNumericCompare, NumericCompare, NumericComparison};
pub use value::complex::{BranchPolicy, Complex};
pub use representation::big_float::{Decimal, RoundingStatus};
/// 有限精度二进制浮点（与 [`Decimal`] 同义；Living 命名对齐）。
pub type BigFloat = Decimal;
pub use representation::domain::NumericDomain;
pub use representation::dyadic::Dyadic;
#[allow(deprecated)]
pub use certificate::NumericProvenance;
pub use certificate::{NumericBinding, NumericEvidenceArena, NumericEvidenceId, NumericEvidenceRecord};
pub use policy::{ExecutionBudget, NumericContext};
pub use value::finite_field::FiniteFieldValue;
pub use value::integer::{ExactInteger, Integer, Sign};
pub use value::interval::{Interval, IntervalDecoration};
pub use arithmetic::kernel_number::{abs, add, compare, div, factorial, mul, neg, pow, sqrt, to_f64_lossy};
pub use value::modular::{ModularValue, Modulus, ModulusBinding, PrimeModulus, ProbablePrimeModulus};
pub use arithmetic::modular_ops::batch_mod_inverse;
pub use arithmetic::modulus_context::{BarrettParams, ModularTimingPolicy, ModulusContext, ModulusTable, MontgomeryParams};
pub use value::number::{Number, NumericValue};
pub use format::wire::{from_wire as number_from_wire, to_wire as number_to_wire};
pub use value::p_adic::PAdicValue;
pub use representation::polynomial_fingerprint::PolynomialFingerprint;
pub use representation::precision::{PrecisionInfo, PrecisionKind};
pub use arithmetic::promotion::{DefaultPromotion, Promotion, PromotionPolicy};
pub use value::rational::{ExactRational, Rational};
pub use value::real::Real;
pub use arithmetic::rounding::RoundingPolicy;
pub use format::serialization::NumericValueWire;
