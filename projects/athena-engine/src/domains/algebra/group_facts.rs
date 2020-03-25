//! 群性质摘要（descriptor 级，非 presentation）。

use athena_numeric::Integer;

use crate::runtime::values::numeric_clone::clone_integer;

use super::property::PropertyState;

/// 群的已知或待证性质集合。
#[derive(Debug, PartialEq, Default)]
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

impl GroupPropertyFacts {
    /// Owning 复制：`Integer` 阶经 GC [`clone_integer`]，不用 Rust [`Clone`] 推导。
    pub fn owning_copy(&self) -> Self {
        Self {
            is_finite: self.is_finite.owning_copy(),
            is_abelian: self.is_abelian.owning_copy(),
            is_solvable: self.is_solvable.owning_copy(),
            order: owning_copy_integer_property(&self.order),
        }
    }
}

pub(crate) fn owning_copy_integer_property(state: &PropertyState<Integer>) -> PropertyState<Integer> {
    state.owning_copy_with(clone_integer)
}
