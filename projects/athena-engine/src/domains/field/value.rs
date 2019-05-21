//! 域论域值。

use super::types::{Field, FieldElement};

/// 域论返回值。
#[derive(Debug, Clone, PartialEq)]
pub enum FieldDomainValue {
    /// 域。
    Field(Field),
    /// 元素。
    Element(FieldElement),
    /// 布尔。
    Boolean(bool),
    /// 占位。
    Placeholder,
}
