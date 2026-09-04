//! 结果覆盖状态与 [`ResultStore`]。

mod store;

pub use store::{ComputationResult, ResultStore};

/// 结果覆盖范围（Living `01` / `26`：禁止用 `complete: bool` 冒充）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoverageStatus {
    /// 声明的能力范围内完整。
    Full,
    /// 已知真子集或未完成分解。
    Partial,
    /// 尚未判定。
    Unknown,
    /// 明确不支持（须显式暴露，禁止回显输入当成功）。
    Unsupported,
}

impl CoverageStatus {
    /// 机器标识。
    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Partial => "Partial",
            Self::Unknown => "Unknown",
            Self::Unsupported => "Unsupported",
        }
    }
}
