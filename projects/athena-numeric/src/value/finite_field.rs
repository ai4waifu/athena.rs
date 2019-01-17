//! 有限域元素骨架。

use athena_types::FieldId;

use crate::value::integer::Integer;

/// 有限域中的元素（canonical 系数 payload）。
///
/// 模多项式、基与约化计划由 engine `FieldPresentation` / `FieldTable` 持有，
/// 不得在本层重复或引用 IR。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteFieldValue {
    /// 域。
    pub field: FieldId,
    /// 约化后的基坐标（长度 < 扩张次数）。
    pub coefficients: Vec<Integer>,
}
