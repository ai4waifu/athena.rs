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
#[allow(deprecated)]
pub use crate::certificate::NumericProvenance;
pub use crate::{
    arithmetic::{
        comparison,
        comparison::{ComparisonPolicy, DefaultNumericCompare, NumericCompare, NumericComparison},
        kernel_number,
        kernel_number::{abs, add, compare, div, factorial, mul, neg, pow, sqrt, to_f64_lossy},
        modular_ops as modular_kernel,
        modular_ops::batch_mod_inverse,
        modulus_context,
        modulus_context::{BarrettParams, ModularTimingPolicy, ModulusContext, ModulusTable, MontgomeryParams},
        promotion,
        promotion::{DefaultPromotion, Promotion, PromotionPolicy},
        rounding,
        rounding::RoundingPolicy,
    },
    backend as backends,
    backend::{
        NumericBackend, NumericBackendContract, NumericBackendLimits, NumericCapability, NumericOperation, NumericResultMode,
        PureRustBackend,
    },
    certificate::{
        CertificateMethod, NumericBinding, NumericCertificate, NumericEvidenceArena, NumericEvidenceId, NumericEvidenceRecord,
    },
    format::{
        binary as wire_binary, serialization,
        serialization::NumericValueWire,
        text as wire_text, wire as number_wire,
        wire::{from_wire as number_from_wire, to_wire as number_to_wire},
    },
    policy::{ExecutionBudget, NumericContext, execution_budget},
    representation::{
        decimal,
        decimal::{Decimal, RoundingStatus},
        domain,
        domain::NumericDomain,
        dyadic,
        dyadic::Dyadic,
        polynomial_fingerprint,
        polynomial_fingerprint::PolynomialFingerprint,
        precision,
        precision::{PrecisionInfo, PrecisionKind},
    },
    value::{
        algebraic,
        algebraic::AlgebraicNumber,
        complex,
        complex::{BranchPolicy, Complex},
        finite_field,
        finite_field::FiniteFieldValue,
        integer,
        integer::{ExactInteger, Integer, Sign},
        interval,
        interval::{Interval, IntervalDecoration},
        modular,
        modular::{ModularValue, Modulus, ModulusBinding, PrimeModulus, ProbablePrimeModulus},
        natural, number,
        number::{Number, NumericValue},
        p_adic,
        p_adic::PAdicValue,
        rational,
        rational::{ExactRational, Rational},
        real,
        real::Real,
    },
};
