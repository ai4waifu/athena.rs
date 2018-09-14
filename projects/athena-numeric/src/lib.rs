//! Athena 数值塔 — 表示、运算、精度、promotion、证书（Living `16` N0 骨架）。
//!
//! `num-*` 仅作内部存储候选，不作为公共语义。公共 API 不暴露 `num_bigint::BigInt`。

#![deny(missing_docs)]

pub mod algebraic;
pub mod backend;
pub mod certificate;
pub mod comparison;
pub mod complex;
pub mod domain;
pub mod finite_field;
pub mod integer;
pub mod interval;
pub mod modular;
pub mod number;
pub mod p_adic;
pub mod precision;
pub mod promotion;
pub mod rational;
pub mod real;
pub mod rounding;
pub mod serialization;

pub use algebraic::AlgebraicNumber;
pub use backend::{NumericBackend, PureRustBackend};
pub use certificate::{CertificateMethod, NumericCertificate};
pub use comparison::{ComparisonPolicy, DefaultNumericCompare, NumericCompare, NumericComparison};
pub use complex::{BranchPolicy, Complex};
pub use domain::NumericDomain;
pub use finite_field::FiniteFieldValue;
pub use integer::Integer;
pub use interval::{Interval, IntervalDecoration};
pub use modular::ModularValue;
pub use number::{NumericProvenance, NumericRepr, NumericValue};
pub use p_adic::PAdicValue;
pub use precision::{PrecisionInfo, PrecisionKind};
pub use promotion::{DefaultPromotion, Promotion, PromotionPolicy};
pub use rational::Rational;
pub use real::Real;
pub use rounding::RoundingPolicy;
pub use serialization::NumericValueWire;
