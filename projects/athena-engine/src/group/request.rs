//! 群论域请求。

use num_bigint::BigInt;

use athena_types::GroupId;

use super::types::{GroupElement, Permutation};

/// 群论域请求（骨架）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupRequest {
    /// 构造循环群。
    Cyclic {
        /// 阶。
        order: BigInt,
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
}
