//! 统一计算状态（Living `24` SEM2）。
//!
//! 禁止各领域用 `complete: bool` 冒充完成度。

/// 跨领域计算 / 结果状态。
///
/// 领域可有更细枚举，但必须能映射到本类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComputationStatus {
    /// 已通过 verifier（具体保证见领域证书）。
    Verified,
    /// 精确且无条件（可进 exact union-find 的候选前提之一）。
    Exact,
    /// 依赖显式假设 / 条件。
    Conditional,
    /// 概率性结论。
    Probable,
    /// 候选，尚未接纳。
    Candidate,
    /// 部分结果（已知真子集或未完成分解）。
    Partial,
    /// 资源截断。
    ResourceLimited,
    /// 未知 / 未判定。
    Unknown,
    /// 输入或状态无效。
    Invalid,
}

impl ComputationStatus {
    /// 是否允许声称「无条件精确完成」。
    pub fn is_unconditional_exact(self) -> bool {
        matches!(self, Self::Exact | Self::Verified)
    }

    /// 是否必须向 renderer / 调用方显式暴露（不可藏成「普通成功」）。
    pub fn must_surface(self) -> bool {
        !matches!(self, Self::Exact | Self::Verified)
    }

    /// 是否表示可恢复截断。
    pub fn is_resource_limited(self) -> bool {
        matches!(self, Self::ResourceLimited)
    }

    /// 机器标识（审计）。
    pub fn name(self) -> &'static str {
        match self {
            Self::Verified => "Verified",
            Self::Exact => "Exact",
            Self::Conditional => "Conditional",
            Self::Probable => "Probable",
            Self::Candidate => "Candidate",
            Self::Partial => "Partial",
            Self::ResourceLimited => "ResourceLimited",
            Self::Unknown => "Unknown",
            Self::Invalid => "Invalid",
        }
    }
}
