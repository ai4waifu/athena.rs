//! 有限域元素骨架。

use athena_types::{FieldId, TermId};

use crate::integer::Integer;

/// 有限域中的元素（系数向量相对模多项式）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteFieldValue {
    /// 域。
    pub field: FieldId,
    /// 系数。
    pub coefficients: Vec<Integer>,
    /// 模多项式引用（骨架）。
    pub modulus_polynomial: TermId,
}
