//! 群与元素对象合同。

use athena_numeric::Integer;
use athena_types::{GroupElementId, GroupId, GroupPresentationId, SubgroupId};

use crate::{
    domains::algebra::{GroupPropertyFacts, PropertyState, owning_copy_integer_property},
    runtime::values::numeric_clone::clone_integer,
};

/// 群数学描述（抽象性质；可运算性取决于 presentation）。
#[derive(Debug, PartialEq)]
pub enum GroupDescriptor {
    /// 仅已知部分性质，尚无具体 presentation 时不可构造元素。
    Abstract {
        /// 阶（若已知）。
        order: PropertyState<Integer>,
        /// 其他性质摘要。
        properties: GroupPropertyFacts,
    },
    /// 置换群：已绑定度数，可经 table-backed presentation 运算。
    Permutation {
        /// 作用度数。
        degree: u32,
    },
}

/// 群对象。
#[derive(Debug, PartialEq)]
pub struct Group {
    /// 稳定 id。
    pub id: GroupId,
    /// 数学描述。
    pub descriptor: GroupDescriptor,
    /// 当前可运算 presentation。
    pub presentation: GroupPresentationId,
    /// 阶（冗余缓存；以 properties 为准）。
    pub order: Option<Integer>,
}

/// 子群 H ≤ G（含包含映射 id）。
#[derive(Debug, PartialEq, Eq)]
pub struct Subgroup {
    /// 稳定 id。
    pub id: SubgroupId,
    /// 父群 G。
    pub parent: GroupId,
    /// 子群作为独立群对象 H。
    pub group: GroupId,
    /// 包含映射 H ↪ G。
    pub inclusion: athena_types::AlgebraMapId,
}

/// 置换：像列表 `π(i) = images[i]`（0-based；度数由 presentation 解释）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct Permutation {
    /// 像（长度须等于 presentation 的 degree）。
    pub images: Vec<u32>,
}

impl Permutation {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { images: self.images.clone() }
    }
}

/// 群元素表示。
#[derive(Debug, PartialEq, Eq)]
pub enum GroupElementRepr {
    /// 仅 [`GroupPresentationKind::ExplicitTable`] 下合法。
    TableIndex(u64),
    /// 置换像。
    Permutation(Permutation),
}

/// 绑定群的元素。
#[derive(Debug, PartialEq, Eq)]
pub struct GroupElement {
    /// 元素 id。
    pub id: GroupElementId,
    /// 所属群。
    pub group: GroupId,
    /// repr 所属的 presentation。
    pub presentation: GroupPresentationId,
    /// 表示。
    pub repr: GroupElementRepr,
}

impl GroupDescriptor {
    /// Owning 复制：`Integer` 经 GC [`clone_integer`]。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Abstract { order, properties } => {
                Self::Abstract { order: owning_copy_integer_property(order), properties: properties.owning_copy() }
            }
            Self::Permutation { degree } => Self::Permutation { degree: *degree },
        }
    }
}

impl Group {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            id: self.id,
            descriptor: self.descriptor.owning_copy(),
            presentation: self.presentation,
            order: self.order.as_ref().map(clone_integer),
        }
    }
}

impl Subgroup {
    /// Owning 复制（仅 id 句柄，无堆数值）。
    pub fn owning_copy(&self) -> Self {
        Self { id: self.id, parent: self.parent, group: self.group, inclusion: self.inclusion }
    }
}

impl GroupElementRepr {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::TableIndex(i) => Self::TableIndex(*i),
            Self::Permutation(p) => Self::Permutation(p.owning_copy()),
        }
    }
}

impl GroupElement {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { id: self.id, group: self.group, presentation: self.presentation, repr: self.repr.owning_copy() }
    }
}
