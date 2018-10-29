//! 代数映射与验证合同。

use athena_types::{AlgebraMapId, PresentationId};

use super::parent::AlgebraParentId;

/// 映射验证策略与结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapVerification {
    /// 验证种类。
    pub kind: MapVerificationKind,
    /// 是否已通过验证（仅 kind 为具体策略时为 true）。
    pub verified: bool,
}

/// 映射验证种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapVerificationKind {
    /// 尚未验证。
    Unverified,
    /// 有限生成：生成元像 + 关系验证。
    GeneratorRelations,
    /// 度数 / 次数一致性检查。
    DegreeCheck,
    /// 外部证书（adapter）。
    ExternalCertificate,
}

/// 映射种类（Phase 0：不含元素 payload，由领域模块填充 images）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlgebraMapKind {
    /// 域嵌入 K → L。
    FieldEmbedding,
    /// 群同态 G → H。
    GroupHomomorphism,
    /// 商投影 G → G/N。
    QuotientProjection,
    /// 子群包含 H ↪ G。
    SubgroupInclusion,
}

/// 统一代数映射。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlgebraMap {
    /// 稳定 id。
    pub id: AlgebraMapId,
    /// 源父对象。
    pub source: AlgebraParentId,
    /// 靶父对象。
    pub target: AlgebraParentId,
    /// 映射种类。
    pub kind: AlgebraMapKind,
    /// 验证状态。
    pub verification: MapVerification,
}

/// 域嵌入（K → L）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldEmbedding {
    /// 底层映射 id。
    pub map: AlgebraMapId,
    /// 解释 generator images 的 presentation。
    pub source_presentation: PresentationId,
    /// 解释 generator images 的 presentation。
    pub target_presentation: PresentationId,
}

/// 群同态（G → H）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupHomomorphism {
    /// 底层映射 id。
    pub map: AlgebraMapId,
    /// 源 presentation。
    pub source_presentation: PresentationId,
    /// 靶 presentation。
    pub target_presentation: PresentationId,
}
