//! 群论域值。

use athena_numeric::Integer;
use athena_types::AlgebraMapId;

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
