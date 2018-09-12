//! 域与域元素对象（骨架）。

use num_bigint::BigInt;

use athena_types::{ExtensionId, FieldId};

/// 域种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    /// 有理数域 ℚ。
    Rationals,
    /// 素域 𝔽_p。
    Prime {
        /// 素数特征。
        characteristic: BigInt,
    },
    /// 有限扩张（模不可约多项式等，细节后续）。
    FiniteExtension {
        /// 基域。
        base: FieldId,
        /// 扩张 id。
        extension: ExtensionId,
        /// 次数（若已知）。
        degree: Option<u32>,
    },
}

/// 域对象。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// 稳定 id。
    pub id: FieldId,
    /// 种类。
    pub kind: FieldKind,
}

/// 域元素（表示后续接多项式/坐标）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldElement {
    /// 所属域。
    pub field: FieldId,
    /// 桥接标签（非正式代数表示）。
    pub label: String,
}
