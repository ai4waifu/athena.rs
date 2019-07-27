//! Athena 数值塔 — 表示、运算、精度、promotion、证书。
//!
//! unsafe 边界：仅 `storage` 窄模块（纯 `union Magnitude` + `OwnedLimbBuffer`）允许 unsafe。
//!
//! 四层正交（Living `13`）：
//! ```text
//! storage    — Magnitude / meta / views
//! algorithm  — 数学策略（`AlgorithmPlanner`）
//! kernel     — portable / x86_64 `KernelTable`（context 级绑定）
//! dispatch   — 能力三分 · `NumericExecutor` · `PortableBackend`
//! foreign    — oracle / native-accelerated（不进默认路径）
//! ```
#![deny(missing_docs)]

pub mod algorithm;
pub mod arithmetic;
pub mod certificate;
pub mod dispatch;
pub mod foreign;
pub mod format;
pub mod kernel;
pub mod policy;
pub mod representation;
pub(crate) mod storage;
pub mod value;

// —— 稳定模块路径（真实模块，不是别名文件）——
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
    certificate::{CertificateMethod, NumericBinding, NumericCertificate, NumericEvidenceArena, NumericEvidenceId, NumericEvidenceRecord},
    dispatch::{
        AlgorithmCapability, CapabilityBundle, MachineCapability, NumericBackend, NumericBackendContract, NumericBackendLimits,
        NumericCapability, NumericExecutor, NumericOperation, NumericResultMode, PortableBackend, ResourceCapability,
    },
    format::{
        binary as wire_binary, serialization,
        serialization::NumericValueWire,
        text as wire_text, wire as number_wire,
        wire::{from_wire as number_from_wire, to_wire as number_to_wire},
    },
    kernel::{ExecutionToken, KernelTable, ScratchWorkspace},
    policy::{CancellationToken, ExecutionBudget, NumericContext, execution_budget},
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
        algebraic::{AlgebraicNumber, AlgebraicRepresentation},
        complex,
        complex::{BranchPolicy, Complex},
        finite_field,
        finite_field::{FiniteFieldRepr, FiniteFieldValue},
        integer,
        integer::{ExactInteger, Integer, MagnitudeView, Sign},
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

#[cfg(feature = "ephemeral")]
pub use value::ephemeral::{EphemeralInteger, EphemeralNatural};
