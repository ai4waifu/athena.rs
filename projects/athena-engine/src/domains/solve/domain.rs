//! Solve 定义域。

use athena_types::{ExtensionId, FieldId, TermId};

use crate::domains::algebra::AlgebraParentId;

/// 求解定义域（必须显式，禁止默认吞掉）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolveDomain {
    /// `ℤ`
    Integers,
    /// `ℚ`
    Rationals,
    /// `ℝ`
    Reals,
    /// `ℂ`
    Complexes,
    /// 有限域（经 [`FieldId`]）。
    FiniteField {
        /// 域句柄。
        field: FieldId,
    },
    /// 代数扩域。
    AlgebraicExtension {
        /// 扩域句柄。
        extension: ExtensionId,
    },
    /// 实区间（端点为 IR 项）。
    Interval {
        /// 下界。
        lo: TermId,
        /// 上界。
        hi: TermId,
        /// 是否含下界。
        closed_lo: bool,
        /// 是否含上界。
        closed_hi: bool,
    },
    /// 用户指定代数父对象。
    Parent {
        /// 父对象。
        parent: AlgebraParentId,
    },
}
