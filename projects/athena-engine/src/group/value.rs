//! 群论域值。

use athena_numeric::Integer;

use super::types::{Group, GroupElement};

/// 群论域返回值。
#[derive(Debug, Clone, PartialEq)]
pub enum GroupDomainValue {
    /// 群对象。
    Group(Group),
    /// 群元素。
    Element(GroupElement),
    /// 布尔性质。
    Boolean(bool),
    /// 整数（阶等）。
    Integer(Integer),
    /// 占位。
    Placeholder,
}
