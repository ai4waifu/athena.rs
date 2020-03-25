//! 非负大整数（`meta` + 纯 `union Magnitude`；算法委托 [`crate::kernel::limb`]）。

mod arithmetic;
mod conversion;
mod owned;
mod publish;
mod query;
mod trace;

use crate::{kernel::limb as limb_kernel, storage::MagnitudePair};
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

/// 自然数（小端 `u64` limb，无尾随零）。
///
/// 布局：`meta`（mode+heap_len；sign 位 don't-care）+ `union Magnitude`，LP64 上 24 bytes。
/// 经私有 [`MagnitudePair`] 做 Drop；读取时不解释 sign，语义恒为非负。
///
/// # 复制合同
///
/// **不**实现 [`Clone`]。Limb1/Limb2 用 [`Self::clone_inline`]；Heap owning 深复制用
/// [`Self::try_clone_in`]。
#[derive(Default)]
pub struct Natural {
    inner: MagnitudePair,
}

impl PartialEq for Natural {
    fn eq(&self, other: &Self) -> bool {
        // 忽略 meta sign/reserved don't-care 位。
        self.as_limbs() == other.as_limbs()
    }
}

impl Eq for Natural {}

impl Hash for Natural {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_limbs().hash(state);
    }
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<Natural>() == 24);
    assert!(core::mem::align_of::<Natural>() == 8);
};

impl core::fmt::Debug for Natural {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Natural").field("limbs", &self.as_limbs()).finish()
    }
}

impl PartialOrd for Natural {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Natural {
    fn cmp(&self, other: &Self) -> Ordering {
        limb_kernel::cmp_slice(self.as_limbs(), other.as_limbs())
    }
}

impl Natural {
    /// 零。
    pub fn zero() -> Self {
        Self { inner: MagnitudePair::zero() }
    }

    /// 一。
    pub fn one() -> Self {
        Self { inner: MagnitudePair::from_u64(1) }
    }

    /// 由 `u64` 构造。
    pub fn from_u64(n: u64) -> Self {
        Self { inner: MagnitudePair::from_u64(n) }
    }
}
