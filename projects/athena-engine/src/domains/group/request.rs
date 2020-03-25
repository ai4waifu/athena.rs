//! 群论域请求。

use athena_numeric::Integer;
use athena_types::{AlgebraMapId, GroupId, SubgroupId};

use crate::runtime::values::numeric_clone::clone_integer;

use super::types::{GroupElement, Permutation};

/// 群论域请求（骨架）。
#[derive(Debug, PartialEq, Eq)]
pub enum GroupRequest {
    /// 构造循环群。
    Cyclic {
        /// 阶。
        order: Integer,
    },
    /// 由生成置换构造置换群。
    PermutationGroup {
        /// 度数。
        degree: u32,
        /// 生成元。
        generators: Vec<Permutation>,
    },
    /// 群阶。
    Order {
        /// 群。
        group: GroupId,
    },
    /// 元素乘法。
    Multiply {
        /// 左。
        lhs: GroupElement,
        /// 右。
        rhs: GroupElement,
    },
    /// 逆元。
    Inverse {
        /// 元素。
        element: GroupElement,
    },
    /// 是否交换。
    IsAbelian {
        /// 群。
        group: GroupId,
    },
    /// 由生成元构造子群。
    SubgroupFromGenerators {
        /// 父群。
        parent: GroupId,
        /// 子群生成元。
        generators: Vec<Permutation>,
    },
    /// 子群是否正规。
    IsNormalSubgroup {
        /// 子群。
        subgroup: SubgroupId,
    },
    /// 构造商群 G/N。
    QuotientGroup {
        /// 正规子群 N。
        subgroup: SubgroupId,
    },
    /// 由生成元像构造同态。
    HomomorphismFromGeneratorImages {
        /// 源群。
        source: GroupId,
        /// 靶群。
        target: GroupId,
        /// 与源群生成元对齐的像。
        generator_images: Vec<Permutation>,
    },
    /// 应用同态。
    ApplyHomomorphism {
        /// 同态映射 id。
        map: AlgebraMapId,
        /// 源群元素。
        element: GroupElement,
    },
    /// 商投影。
    ProjectQuotient {
        /// 子群 N（须正规且已构造商群）。
        subgroup: SubgroupId,
        /// 父群元素。
        element: GroupElement,
    },
}

impl GroupRequest {
    /// Owning 复制：`Integer` 经 GC [`clone_integer`]，元素经 [`GroupElement::owning_copy`]。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Cyclic { order } => Self::Cyclic { order: clone_integer(order) },
            Self::PermutationGroup { degree, generators } => {
                Self::PermutationGroup { degree: *degree, generators: generators.iter().map(Permutation::owning_copy).collect() }
            }
            Self::Order { group } => Self::Order { group: *group },
            Self::Multiply { lhs, rhs } => Self::Multiply { lhs: lhs.owning_copy(), rhs: rhs.owning_copy() },
            Self::Inverse { element } => Self::Inverse { element: element.owning_copy() },
            Self::IsAbelian { group } => Self::IsAbelian { group: *group },
            Self::SubgroupFromGenerators { parent, generators } => {
                Self::SubgroupFromGenerators { parent: *parent, generators: generators.iter().map(Permutation::owning_copy).collect() }
            }
            Self::IsNormalSubgroup { subgroup } => Self::IsNormalSubgroup { subgroup: *subgroup },
            Self::QuotientGroup { subgroup } => Self::QuotientGroup { subgroup: *subgroup },
            Self::HomomorphismFromGeneratorImages { source, target, generator_images } => Self::HomomorphismFromGeneratorImages {
                source: *source,
                target: *target,
                generator_images: generator_images.iter().map(Permutation::owning_copy).collect(),
            },
            Self::ApplyHomomorphism { map, element } => Self::ApplyHomomorphism { map: *map, element: element.owning_copy() },
            Self::ProjectQuotient { subgroup, element } => Self::ProjectQuotient { subgroup: *subgroup, element: element.owning_copy() },
        }
    }
}
