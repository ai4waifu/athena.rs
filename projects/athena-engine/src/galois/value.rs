//! 伽罗瓦域值对象。

use athena_types::{AlgebraMapId, AutomorphismId, ExtensionId, FieldId, GroupId, SubgroupId};

use crate::algebra::PropertyState;

/// 域自同构（L → L 的特殊 field embedding）。
#[derive(Debug, Clone, PartialEq)]
pub struct FieldAutomorphism {
    /// 作用的扩张。
    pub extension: ExtensionId,
    /// 底层嵌入映射。
    pub embedding: AlgebraMapId,
    /// 是否固定基域（须带 witness）。
    pub fixes_base: PropertyState<bool>,
    /// 逆自同构（若已知）。
    pub inverse: Option<AutomorphismId>,
}

/// 向后兼容别名。
pub type Automorphism = FieldAutomorphism;

/// 伽罗瓦群计算状态（禁止单一 `complete: bool` 冒充完整结果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GaloisComputation {
    /// 完整且已验证的伽罗瓦群。
    Complete {
        /// 群 id。
        group: GroupId,
    },
    /// 候选子群（上下界收紧中）。
    CandidateSubgroup {
        /// 当前候选。
        candidate: GroupId,
    },
    /// 认证包含关系：lower ≤ Gal ≤ upper。
    CertifiedContainment {
        /// 下界子群。
        lower: GroupId,
        /// 上界子群。
        upper: GroupId,
    },
    /// 已找到部分自同构。
    Partial {
        /// 已找到自同构数量。
        automorphisms_found: u32,
    },
    /// 资源截断。
    ResourceLimited,
}

/// 伽罗瓦群结果。
#[derive(Debug, Clone, PartialEq)]
pub struct GaloisGroup {
    /// 基域。
    pub base_field: FieldId,
    /// 计算状态。
    pub computation: GaloisComputation,
}

/// 伽罗瓦域返回值。
#[derive(Debug, Clone, PartialEq)]
pub enum GaloisDomainValue {
    /// 布尔性质（可分 / 正规等）。
    Boolean(bool),
    /// 自同构。
    Automorphism(FieldAutomorphism),
    /// 伽罗瓦群。
    GaloisGroup(GaloisGroup),
    /// 固定域子群 id（占位；完整 fixed field 后续接 FieldId）。
    FixedFieldSubgroup(SubgroupId),
    /// 占位。
    Placeholder,
}
