//! 系数环与除法策略。

use athena_types::{FieldId, Modulus};

/// 第一阶段系数环标识（非泛型擦除入口）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoefficientRing {
    /// ℤ。
    Integer,
    /// ℚ。
    Rational,
    /// 机器 / 任意精度实数（后续）。
    Real,
    /// 有限域 `𝔽_q`（经 [`FieldId`]）。
    FiniteField(FieldId),
    /// ℤ/nℤ。
    ModularInteger(Modulus),
}

/// 显式除法策略 — `ℤ[x]` 不得无条件域除。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DivisionPolicy {
    /// 仅精确整除。
    #[default]
    ExactOnly,
    /// 系数域上的域除法。
    FieldDivision,
    /// 伪除。
    PseudoDivision,
    /// 提升到有理系数（须写入 metadata）。
    PromoteToRational,
}
