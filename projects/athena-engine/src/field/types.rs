//! 域与域元素对象合同。

use athena_numeric::{Integer, Rational};
use athena_types::{ExtensionId, FieldId, PresentationId};

use crate::algebra::PropertyState;

/// 域对象。
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// 稳定 id。
    pub id: FieldId,
    /// 数学描述（与 presentation 分离）。
    pub descriptor: FieldDescriptor,
    /// 默认 presentation。
    pub presentation: PresentationId,
}

/// 域元素表示（按 presentation kind 解释）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldElementRepr {
    /// ℚ：约分后、分母为正的有理数 payload。
    Rational {
        /// canonical [`Rational`]。
        value: Rational,
    },
    /// 𝔽_p：约化后的 residue。
    PrimeFieldResidue {
        /// 值 ∈ [0, p)。
        value: Integer,
    },
    /// 扩张：次数小于 defining polynomial 的系数向量。
    ExtensionCoords {
        /// 基坐标。
        coords: Vec<Integer>,
    },
    /// 占位。
    Placeholder,
}

/// 域元素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldElement {
    /// 所属域。
    pub field: FieldId,
    /// 解释 repr 的 presentation。
    pub presentation: PresentationId,
    /// 私有表示。
    pub repr: FieldElementRepr,
}

/// 域数学描述（种类与表示分离）。
#[derive(Debug, Clone, PartialEq)]
pub enum FieldDescriptor {
    /// 有理数域 ℚ。
    Rationals,
    /// 素域 𝔽_p。
    Prime {
        /// 素数特征。
        characteristic: Integer,
    },
    /// 有限扩张 K ↪ L。
    Extension {
        /// 基域。
        base: FieldId,
        /// 扩张 id。
        extension: ExtensionId,
        /// 次数（若已知）。
        degree: PropertyState<u32>,
    },
}

/// 向后兼容别名（迁移期；新代码用 [`FieldDescriptor`]）。
pub type FieldKind = FieldDescriptor;
