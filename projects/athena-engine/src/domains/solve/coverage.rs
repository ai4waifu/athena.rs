//! 覆盖状态（禁止用 `complete: bool` 冒充）。

use super::frontier::ResumeToken;

/// 解集覆盖承诺。
///
/// 只有 [`CoverageStatus::Complete`] 或经证明的
/// [`CoverageStatus::CompleteUnderAssumptions`] 才能声称覆盖指定域内全部解。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub enum CoverageStatus {
    /// 在指定 domain / assumptions / policy 下已证明覆盖全部解。
    Complete,
    /// 在显式 assumptions 下完整。
    CompleteUnderAssumptions,
    /// 已证明是真解子集，可能遗漏。
    CertifiedSubset,
    /// 已证明是超集（含伪根风险）。
    CertifiedSuperset,
    /// 仅局部（初值 / 邻域依赖）。
    LocalOnly,
    /// 概率性完整。
    Probable,
    /// 资源截断，携带可恢复前沿。
    ResourceLimited {
        /// 可恢复前沿。
        frontier: ResumeToken,
    },
    /// 当前不支持该 goal / domain 组合。
    Unsupported,
    /// 问题本身无效。
    Invalid,
}

impl CoverageStatus {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Complete => Self::Complete,
            Self::CompleteUnderAssumptions => Self::CompleteUnderAssumptions,
            Self::CertifiedSubset => Self::CertifiedSubset,
            Self::CertifiedSuperset => Self::CertifiedSuperset,
            Self::LocalOnly => Self::LocalOnly,
            Self::Probable => Self::Probable,
            Self::ResourceLimited { frontier } => Self::ResourceLimited { frontier: frontier.owning_copy() },
            Self::Unsupported => Self::Unsupported,
            Self::Invalid => Self::Invalid,
        }
    }

    /// 是否允许进入 exact union-find / 无条件 exact rewrite。
    pub fn admits_exact_union_find(&self) -> bool {
        matches!(self, Self::Complete | Self::CompleteUnderAssumptions)
    }

    /// 前端 renderer 是否必须显式暴露覆盖状态（不可隐藏）。
    pub fn must_surface_to_renderer(&self) -> bool {
        !matches!(self, Self::Complete)
    }

    /// 是否携带可恢复前沿。
    pub fn resume_token(&self) -> Option<&ResumeToken> {
        match self {
            Self::ResourceLimited { frontier } => Some(frontier),
            _ => None,
        }
    }
}
