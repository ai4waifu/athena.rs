//! 伽罗瓦域值对象。

use athena_types::{AlgebraMapId, AutomorphismId, ExtensionId, FieldId, GroupId, SubgroupId};

use crate::domains::algebra::PropertyState;

/// 域自同构：L → L 的特殊嵌入，固定基域。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct FieldAutomorphism {
    /// 稳定 id。
    pub id: AutomorphismId,
    /// 作用的扩张。
    pub extension: ExtensionId,
    /// 底层 field embedding（L → L）。
    pub embedding: AlgebraMapId,
    /// 是否固定基域（须已证明）。
    pub fixes_base: PropertyState<bool>,
    /// 逆自同构（若已知）。
    pub inverse: Option<AutomorphismId>,
}

impl FieldAutomorphism {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            id: self.id,
            extension: self.extension,
            embedding: self.embedding,
            fixes_base: self.fixes_base.owning_copy(),
            inverse: self.inverse,
        }
    }
}

/// 伽罗瓦群计算结果完整性。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum GaloisComputation {
    /// 完整算出并验证。
    Complete {
        /// 伽罗瓦群 id。
        group: GroupId,
    },
    /// 候选子群（尚未证明等于伽罗瓦群）。
    CandidateSubgroup {
        /// 候选群 id。
        group: GroupId,
    },
    /// certified 上下界。
    CertifiedContainment {
        /// 下界（已知包含）。
        lower: GroupId,
        /// 上界（已知包含于）。
        upper: GroupId,
    },
    /// 部分自同构已找到。
    Partial {
        /// 已找到的自同构 id。
        automorphisms: Vec<AutomorphismId>,
    },
    /// 资源截断。
    ResourceLimited {
        /// 已处理边界描述。
        frontier: String,
    },
}

impl GaloisComputation {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Complete { group } => Self::Complete { group: *group },
            Self::CandidateSubgroup { group } => Self::CandidateSubgroup { group: *group },
            Self::CertifiedContainment { lower, upper } => Self::CertifiedContainment { lower: *lower, upper: *upper },
            Self::Partial { automorphisms } => Self::Partial { automorphisms: automorphisms.clone() },
            Self::ResourceLimited { frontier } => Self::ResourceLimited { frontier: frontier.clone() },
        }
    }
}

/// 伽罗瓦群（相对基域与扩张）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct GaloisGroup {
    /// 基域。
    pub base_field: FieldId,
    /// 扩张 id（多项式入口时后续填充）。
    pub extension: Option<ExtensionId>,
    /// 计算状态（禁止单一 complete bool）。
    pub computation: GaloisComputation,
}

impl GaloisGroup {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { base_field: self.base_field, extension: self.extension, computation: self.computation.owning_copy() }
    }
}

/// 伽罗瓦域返回值。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum GaloisDomainValue {
    /// 布尔性质（可分 / 正规等）。
    Boolean(bool),
    /// 自同构。
    Automorphism(FieldAutomorphism),
    /// 伽罗瓦群。
    GaloisGroup(GaloisGroup),
    /// 固定域（子群 → 中间域；`FieldId` 由 Galois 分派填充）。
    FixedField {
        /// 扩张。
        extension: ExtensionId,
        /// 自同构子群。
        subgroup: SubgroupId,
    },
    /// 占位。
    Placeholder,
}

impl GaloisDomainValue {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Boolean(b) => Self::Boolean(*b),
            Self::Automorphism(a) => Self::Automorphism(a.owning_copy()),
            Self::GaloisGroup(g) => Self::GaloisGroup(g.owning_copy()),
            Self::FixedField { extension, subgroup } => Self::FixedField { extension: *extension, subgroup: *subgroup },
            Self::Placeholder => Self::Placeholder,
        }
    }
}
