//! 群性质摘要（descriptor 级，非 presentation）。

use athena_numeric::Integer;

use super::property::PropertyState;

/// 群的已知或待证性质集合。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GroupPropertyFacts {
    /// 是否有限。
    pub is_finite: PropertyState<bool>,
    /// 是否交换。
    pub is_abelian: PropertyState<bool>,
    /// 是否可解。
    pub is_solvable: PropertyState<bool>,
    /// 阶。
    pub order: PropertyState<Integer>,
}
