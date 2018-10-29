//! 域与域元素对象（骨架）。

use athena_numeric::Integer;
use athena_types::{ExtensionId, FieldId, PresentationId};

/// 域种类（descriptor 级；具体表示见 [`crate::algebra::FieldPresentation`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    /// 有理数域 ℚ。
    Rationals,
    /// 素域 𝔽_p。
    Prime {
        /// 素数特征。
        characteristic: Integer,
    },
    /// 有限扩张（模不可约多项式等，细节后续）。
    FiniteExtension {
        /// 基域。
        base: FieldId,
        /// 扩张 id。
        extension: ExtensionId,
        /// 次数（若已知）。
        degree: Option<u32>,
    },
}

/// 域对象。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// 稳定 id。
    pub id: FieldId,
    /// 种类。
    pub kind: FieldKind,
    /// 默认 presentation（Phase 0 可选）。
    pub default_presentation: Option<PresentationId>,
}

/// 域元素私有表示（按 presentation kind 解释）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldElementRepr {
    /// 有理数 canonical 分数（Phase 1 接 `Rational`）。
    Rational {
        /// 分子。
        numerator: Integer,
        /// 分母（须为正）。
        denominator: Integer,
    },
    /// 素域 𝔽_p 约化 residue。
    PrimeFieldResidue {
        /// 约化后的值。
        value: Integer,
    },
    /// 扩张元素系数向量（长度 < degree，相对固定基）。
    ExtensionCoefficients {
        /// 基坐标。
        coefficients: Vec<Integer>,
    },
    /// 占位。
    Placeholder,
}

/// 域元素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldElement {
    /// 所属域。
    pub field: FieldId,
    /// 解释 `repr` 的 presentation。
    pub presentation: PresentationId,
    /// 私有表示 payload。
    pub repr: FieldElementRepr,
}
