//! 数学父对象身份。

use athena_types::{CoefficientRingId, FieldId, GroupId, RingId};

/// 跨环 / 域 / 群的统一父对象句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlgebraParentId {
    /// 多项式环或其他环。
    Ring(RingId),
    /// 域。
    Field(FieldId),
    /// 群。
    Group(GroupId),
}

/// 多项式系数父对象（Living `18` Phase 2：`RingDescriptor.coefficients` 真相源）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoefficientParent {
    /// 系数环（ℤ、ℚ、ℤ/nℤ …；经 [`CoefficientRingId`] intern）。
    Ring(CoefficientRingId),
    /// 域系数（𝔽_p、𝔽_{p^n} …；经 [`FieldId`] intern）。
    Field(FieldId),
}

impl CoefficientParent {
    /// 提升到统一父对象 id（系数环尚未映射到 [`AlgebraParentId::Ring`]）。
    pub fn as_algebra_parent(self) -> Option<AlgebraParentId> {
        match self {
            Self::Ring(_) => None,
            Self::Field(id) => Some(AlgebraParentId::Field(id)),
        }
    }
}
