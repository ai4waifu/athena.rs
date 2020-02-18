//! 域论域请求。

use athena_numeric::Integer;
use athena_types::FieldId;

use crate::runtime::values::numeric_clone::clone_integer;

use super::types::FieldElement;

/// 域论域请求（骨架）。
#[derive(Debug, PartialEq, Eq)]
pub enum FieldRequest {
    /// 素域 𝔽_p。
    PrimeField {
        /// 素数 p。
        characteristic: Integer,
    },
    /// 有理数域。
    Rationals,
    /// 元素加法。
    Add {
        /// 左。
        lhs: FieldElement,
        /// 右。
        rhs: FieldElement,
    },
    /// 元素乘法。
    Mul {
        /// 左。
        lhs: FieldElement,
        /// 右。
        rhs: FieldElement,
    },
    /// 逆元。
    Inverse {
        /// 元素。
        element: FieldElement,
    },
    /// 查询域对象（Session 占位）。
    Lookup {
        /// 域 id。
        field: FieldId,
    },
}

impl FieldRequest {
    /// Owning 复制：`Integer` 经 GC [`clone_integer`]，元素经 [`FieldElement::owning_copy`]。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::PrimeField { characteristic } => Self::PrimeField { characteristic: clone_integer(characteristic) },
            Self::Rationals => Self::Rationals,
            Self::Add { lhs, rhs } => Self::Add { lhs: lhs.owning_copy(), rhs: rhs.owning_copy() },
            Self::Mul { lhs, rhs } => Self::Mul { lhs: lhs.owning_copy(), rhs: rhs.owning_copy() },
            Self::Inverse { element } => Self::Inverse { element: element.owning_copy() },
            Self::Lookup { field } => Self::Lookup { field: *field },
        }
    }
}

impl Clone for FieldRequest {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}
