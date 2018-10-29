//! 数学父对象身份。

use athena_types::{FieldId, GroupId, RingId};

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

/// 多项式系数父对象（迁移目标，见 Living `18` §CoefficientDomain deprecation）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoefficientParent {
    /// 特征 0 环等（ℤ、ℚ、ℤ/nℤ …）。
    Ring(RingId),
    /// 域系数（𝔽_p、𝔽_{p^n}、数域 …）。
    Field(FieldId),
}

impl CoefficientParent {
    /// 提升到统一父对象 id。
    pub fn as_algebra_parent(self) -> AlgebraParentId {
        match self {
            Self::Ring(id) => AlgebraParentId::Ring(id),
            Self::Field(id) => AlgebraParentId::Field(id),
        }
    }
}
