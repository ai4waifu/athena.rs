//! 域与域元素对象合同。

use crate::runtime::values::numeric_clone::{clone_integer, clone_integers, clone_rational, clone_rationals};
use athena_numeric::{Integer, Rational};
use athena_types::{ExtensionId, FieldId, FieldPresentationId};

use crate::domains::algebra::PropertyState;

/// 域对象。
#[derive(Debug, PartialEq)]
pub struct Field {
    /// 稳定 id。
    pub id: FieldId,
    /// 数学描述（与 presentation 分离）。
    pub descriptor: FieldDescriptor,
    /// 默认 presentation。
    pub presentation: FieldPresentationId,
}

/// 域元素表示（按 presentation kind 解释）。
#[derive(Debug, PartialEq, Eq)]
pub enum FieldElementRepr {
    /// ℚ：约分后、分母为正的有理数 payload。
    Rational {
        /// canonical 有理数 [`Rational`]。
        value: Rational,
    },
    /// 𝔽_p：约化后的 residue。
    PrimeFieldResidue {
        /// 值 ∈ [0, p)。
        value: Integer,
    },
    /// 扩张：有限域多项式基坐标（系数 ∈ 𝔽_p）。
    ExtensionCoords {
        /// 基坐标。
        coords: Vec<Integer>,
    },
    /// 数域幂基坐标（系数 ∈ ℚ）。
    NumberFieldCoords {
        /// 绝对幂基坐标。
        coords: Vec<Rational>,
    },
    /// 占位。
    Placeholder,
}

/// 域元素。
#[derive(Debug, PartialEq, Eq)]
pub struct FieldElement {
    /// 所属域。
    pub field: FieldId,
    /// 解释 repr 的 presentation。
    pub presentation: FieldPresentationId,
    /// 私有表示。
    pub repr: FieldElementRepr,
}

/// 域数学描述（种类与表示分离）。
#[derive(Debug, PartialEq)]
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

impl FieldDescriptor {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Rationals => Self::Rationals,
            Self::Prime { characteristic } => Self::Prime { characteristic: clone_integer(characteristic) },
            Self::Extension { base, extension, degree } => Self::Extension { base: *base, extension: *extension, degree: degree.clone() },
        }
    }
}

impl Clone for FieldDescriptor {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}

impl Field {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { id: self.id, descriptor: self.descriptor.owning_copy(), presentation: self.presentation }
    }
}

impl Clone for Field {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}

impl FieldElementRepr {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Rational { value } => Self::Rational { value: clone_rational(value) },
            Self::PrimeFieldResidue { value } => Self::PrimeFieldResidue { value: clone_integer(value) },
            Self::ExtensionCoords { coords } => Self::ExtensionCoords { coords: clone_integers(coords) },
            Self::NumberFieldCoords { coords } => Self::NumberFieldCoords { coords: clone_rationals(coords) },
            Self::Placeholder => Self::Placeholder,
        }
    }
}

impl Clone for FieldElementRepr {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}

impl FieldElement {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { field: self.field, presentation: self.presentation, repr: self.repr.owning_copy() }
    }
}

impl Clone for FieldElement {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}
