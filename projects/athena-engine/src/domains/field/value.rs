//! 域论域值。

use super::types::{Field, FieldElement};

/// 域论返回值。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
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

impl FieldDomainValue {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Field(f) => Self::Field(f.owning_copy()),
            Self::Element(e) => Self::Element(e.owning_copy()),
            Self::Boolean(b) => Self::Boolean(*b),
            Self::Placeholder => Self::Placeholder,
        }
    }
}
