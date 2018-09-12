//! 群与元素对象合同（骨架）。

use num_bigint::BigInt;

use athena_types::{GroupElementId, GroupId};

/// 群种类（第一阶段：有限 / 置换 / 循环）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupKind {
    /// 抽象有限群（内部 index 表示）。
    Finite {
        /// 阶。
        order: BigInt,
    },
    /// 置换群（对称群子群）。
    Permutation {
        /// 作用度数。
        degree: u32,
    },
    /// 循环群。
    Cyclic {
        /// 阶。
        order: BigInt,
    },
}

/// 群对象。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// 稳定 id。
    pub id: GroupId,
    /// 种类。
    pub kind: GroupKind,
    /// 阶（若已知）。
    pub order: Option<BigInt>,
}

/// 置换：像列表 `π(i) = images[i]`（0-based）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permutation {
    /// 像。
    pub images: Vec<u32>,
}

/// 群元素表示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupElementRepr {
    /// 有限群内部 canonical index。
    Index(u64),
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
    /// 表示。
    pub repr: GroupElementRepr,
}
