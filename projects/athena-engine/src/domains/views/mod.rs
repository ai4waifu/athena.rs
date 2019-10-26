//! 跨领域 TypedView（Living `28`）。
//!
//! View 只读、带 fingerprint / revision，不拥有 DomainObject payload。
//! 禁止领域间通过 `Vec` 全量复制或裸 `TermId` 冒充跨域对象。

mod series_polynomial;

pub use series_polynomial::SeriesPolynomialView;

use crate::reasoning::mgraph::ObjectRef;

/// View 内容指纹（provisional；稳定算法后续冻结）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewFingerprint(pub u64);

/// View 相对源对象的修订号（源对象变更则递增）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ViewRevision(pub u64);

/// 租约集合占位（chunk / pin / spill 后续挂接 Living `21`/`24`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LeaseSet {
    /// 非空表示仍依赖运行时租约（当前脚手架恒为空）。
    pub active: bool,
}

/// 跨领域视图种类（闭合枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewKind {
    /// 级数前缀 → 多项式风格系数/幂次投影。
    SeriesPolynomial,
    /// 多项式集 → Macaulay / 稀疏矩阵投影（后续切片）。
    PolynomialMatrix,
    /// 图快照 → 稀疏矩阵语义投影（后续切片）。
    GraphMatrix,
}

/// TypedView 公共头（Living `28` `CrossDomainView`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypedViewHeader {
    /// 源 DomainObject。
    pub source: ObjectRef,
    /// 视图种类。
    pub kind: ViewKind,
    /// 源对象修订（脚手架：与 fingerprint 同步派生）。
    pub source_revision: ViewRevision,
    /// 视图指纹。
    pub fingerprint: ViewFingerprint,
    /// 生命周期租约。
    pub lease: LeaseSet,
}

impl TypedViewHeader {
    /// 构造头。
    pub const fn new(
        source: ObjectRef,
        kind: ViewKind,
        source_revision: ViewRevision,
        fingerprint: ViewFingerprint,
    ) -> Self {
        Self {
            source,
            kind,
            source_revision,
            fingerprint,
            lease: LeaseSet { active: false },
        }
    }
}
