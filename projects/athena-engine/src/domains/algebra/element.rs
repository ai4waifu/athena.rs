//! 元素合同与来源追踪。

use athena_types::PresentationId;

use super::parent::AlgebraParentId;

/// 元素构造来源（provenance 追踪）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ElementProvenance {
    /// 经 canonical 化入口构造。
    #[default]
    Canonical,
    /// 经已验证映射迁入。
    Mapped,
    /// 用户 / 方言输入，尚未 canonical 化。
    Unchecked,
}

/// 跨域元素共享形状（具体 repr 由 presentation kind 解释）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlgebraElement {
    /// 所属父对象。
    pub parent: AlgebraParentId,
    /// 解释 `repr` 的表示 id。
    pub presentation: PresentationId,
    /// 构造来源。
    pub provenance: ElementProvenance,
}
