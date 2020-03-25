//! 群论域值。

use athena_numeric::Integer;
use athena_types::AlgebraMapId;

use crate::runtime::values::numeric_clone::clone_integer;

use super::types::{Group, GroupElement, Subgroup};

/// 群论域返回值。
#[derive(Debug, PartialEq)]
pub enum GroupDomainValue {
    /// 群对象。
    Group(Group),
    /// 群元素。
    Element(GroupElement),
    /// 布尔性质。
    Boolean(bool),
    /// 整数（阶等）。
    Integer(Integer),
    /// 子群对象。
    Subgroup(Subgroup),
    /// 代数映射 id（同态等）。
    AlgebraMap(AlgebraMapId),
    /// 占位。
    Placeholder,
}

impl GroupDomainValue {
    /// Owning 复制：`Integer` 经 GC [`clone_integer`]。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Group(g) => Self::Group(g.owning_copy()),
            Self::Element(e) => Self::Element(e.owning_copy()),
            Self::Boolean(b) => Self::Boolean(*b),
            Self::Integer(n) => Self::Integer(clone_integer(n)),
            Self::Subgroup(s) => Self::Subgroup(s.owning_copy()),
            Self::AlgebraMap(id) => Self::AlgebraMap(*id),
            Self::Placeholder => Self::Placeholder,
        }
    }
}
