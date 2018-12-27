//! 群与元素对象合同。

use athena_numeric::Integer;
use athena_types::{GroupElementId, GroupId, GroupPresentationId, SubgroupId};

use crate::algebra::{GroupPropertyFacts, PropertyState};

/// 群数学描述（抽象性质；可运算性取决于 presentation）。
#[derive(Debug, Clone, PartialEq)]
pub enum GroupDescriptor {
    /// 仅已知部分性质，尚无具体 presentation 时不可构造元素。
    Abstract {
        /// 阶（若已知）。
        order: PropertyState<Integer>,
        /// 其他性质摘要。
        properties: GroupPropertyFacts,
    },
}

/// 群对象。
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permutation {
    /// 像（长度须等于 presentation 的 degree）。
    pub images: Vec<u32>,
}

/// 群元素表示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupElementRepr {
    /// 仅 [`GroupPresentationKind::ExplicitTable`] 下合法。
    TableIndex(u64),
    /// 置换像。
    Permutation(Permutation),
}

/// 绑定群的元素。
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// 向后兼容别名（迁移期；新代码用 [`GroupDescriptor`]）。
pub type GroupKind = GroupDescriptor;
