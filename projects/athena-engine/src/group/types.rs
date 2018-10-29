//! 群与元素对象合同（骨架）。

use athena_numeric::Integer;
use athena_types::{GroupElementId, GroupId, PresentationId};

use crate::algebra::{GroupPropertyFacts, PropertyState};

/// 群数学描述（抽象性质；**不可**仅凭阶运算）。
#[derive(Debug, Clone, PartialEq)]
pub enum GroupDescriptor {
    /// 仅已知部分性质，尚无可运算 presentation。
    Abstract {
        /// 阶（若已知）。
        order: PropertyState<Integer>,
        /// 其他性质。
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
    /// 默认可运算 presentation（若有）。
    pub default_presentation: Option<PresentationId>,
}

/// 置换：像列表 `π(i) = images[i]`（0-based）；度数由 presentation 持有。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permutation {
    /// 像（长度须等于 presentation 的 degree）。
    pub images: Vec<u32>,
}

/// 群元素表示（须在明确 presentation 下解释）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupElementRepr {
    /// 显式乘法表 index — **仅** `ExplicitTable` presentation 合法。
    TableIndex(u64),
    /// 置换。
    Permutation(Permutation),
}

/// 绑定群的元素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupElement {
    /// 元素 id。
    pub id: GroupElementId,
    /// 所属群。
    pub group: GroupId,
    /// 解释 `repr` 的 presentation。
    pub presentation: PresentationId,
    /// 表示 payload。
    pub repr: GroupElementRepr,
}
