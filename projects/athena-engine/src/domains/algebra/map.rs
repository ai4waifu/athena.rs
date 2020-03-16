//! 代数映射与验证合同。

use athena_types::{AlgebraMapId, Diagnostic, DiagnosticCode, FieldPresentationId, GroupPresentationId, SubgroupId};

use super::{
    parent::AlgebraParentId,
    property::{PropertyState, PropertyWitness},
};

/// 映射验证种类（策略标签；结果见 [`MapVerification::status`]）。
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

/// 映射验证状态（禁止薄 `verified: bool`）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct MapVerification {
    /// 验证种类。
    pub kind: MapVerificationKind,
    /// 证明态（`()` 载荷：仅关心是否已证）。
    pub status: PropertyState<()>,
}

impl MapVerification {
    /// 未验证。
    pub fn unverified() -> Self {
        Self { kind: MapVerificationKind::Unverified, status: PropertyState::Unknown }
    }

    /// 已证明成立。
    pub fn proven(kind: MapVerificationKind, witness: PropertyWitness) -> Self {
        Self { kind, status: PropertyState::Proven { value: (), witness } }
    }

    /// 已否证。
    pub fn disproven(kind: MapVerificationKind, witness: PropertyWitness) -> Self {
        Self { kind, status: PropertyState::Disproven { witness } }
    }

    /// 是否已证明。
    pub fn is_proven(&self) -> bool {
        self.status.is_proven()
    }

    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            kind: self.kind,
            status: self.status.owning_copy(),
        }
    }
}

/// 映射种类（不含元素 payload，images 由领域模块填充）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
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

impl AlgebraMap {
    /// 要求映射已验证，否则诊断。
    pub fn require_proven(&self) -> athena_types::Result<()> {
        if self.verification.is_proven() {
            Ok(())
        }
        else {
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "algebra")
                .detail("operation", "map_not_proven")
                .detail("map_id", self.id.0.to_string()))
        }
    }

    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            id: self.id,
            source: self.source,
            target: self.target,
            kind: self.kind,
            verification: self.verification.owning_copy(),
        }
    }
}

/// 域嵌入（K → L）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct FieldEmbedding {
    /// 底层映射 id。
    pub map: AlgebraMapId,
    /// 解释 generator images 的 presentation。
    pub source_presentation: FieldPresentationId,
    /// 解释 generator images 的 presentation。
    pub target_presentation: FieldPresentationId,
}

impl FieldEmbedding {
    /// Owning 复制（Living `31`：仅 id 句柄）。
    pub fn owning_copy(&self) -> Self {
        Self {
            map: self.map,
            source_presentation: self.source_presentation,
            target_presentation: self.target_presentation,
        }
    }
}

/// 群同态（G → H）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct GroupHomomorphism {
    /// 底层映射 id。
    pub map: AlgebraMapId,
    /// 源 presentation。
    pub source_presentation: GroupPresentationId,
    /// 靶 presentation。
    pub target_presentation: GroupPresentationId,
}

impl GroupHomomorphism {
    /// Owning 复制（Living `31`：仅 id 句柄）。
    pub fn owning_copy(&self) -> Self {
        Self {
            map: self.map,
            source_presentation: self.source_presentation,
            target_presentation: self.target_presentation,
        }
    }
}

/// 子群包含 H ↪ G。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct SubgroupInclusion {
    /// 底层映射 id。
    pub map: AlgebraMapId,
    /// 子群 id。
    pub subgroup: SubgroupId,
    /// 子群 presentation。
    pub source_presentation: GroupPresentationId,
    /// 父群 presentation。
    pub target_presentation: GroupPresentationId,
}

impl SubgroupInclusion {
    /// Owning 复制（Living `31`：仅 id 句柄）。
    pub fn owning_copy(&self) -> Self {
        Self {
            map: self.map,
            subgroup: self.subgroup,
            source_presentation: self.source_presentation,
            target_presentation: self.target_presentation,
        }
    }
}

/// 商投影 G → G/N。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct QuotientProjection {
    /// 底层映射 id。
    pub map: AlgebraMapId,
    /// 正规子群 id。
    pub subgroup: SubgroupId,
    /// 源 presentation。
    pub source_presentation: GroupPresentationId,
    /// 商 presentation。
    pub target_presentation: GroupPresentationId,
}

impl QuotientProjection {
    /// Owning 复制（Living `31`：仅 id 句柄）。
    pub fn owning_copy(&self) -> Self {
        Self {
            map: self.map,
            subgroup: self.subgroup,
            source_presentation: self.source_presentation,
            target_presentation: self.target_presentation,
        }
    }
}
