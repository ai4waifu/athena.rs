//! Fixture 校验摘要（正确性优先于性能）。

use serde::Serialize;

/// 精确性声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactnessKind {
    /// 精确整数 / 有理等。
    Exact,
    /// 机器浮点。
    Machine,
    /// 混合或不适用。
    Mixed,
    /// 未声明（占位 fixture）。
    Unspecified,
}

/// 确定性保证摘要。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterminacyKind {
    /// 相同输入应产生相同结果。
    Deterministic,
    /// 允许非确定（当前基准默认不用）。
    Nondeterministic,
    /// 未声明。
    Unspecified,
}

/// 校验通过后写入报告的摘要。
#[derive(Debug, Clone, Serialize)]
pub struct ValidationSummary {
    /// 是否通过结果校验。
    pub ok: bool,
    /// 精确性。
    pub exactness: ExactnessKind,
    /// 确定性。
    pub determinacy: DeterminacyKind,
    /// 观测到的 `ATHENA_*` diagnostic code（通常为空）。
    pub diagnostic_codes: Vec<String>,
    /// 人类可读短摘要。
    pub notes: String,
}

impl ValidationSummary {
    /// 构造通过摘要。
    pub fn passed(exactness: ExactnessKind, determinacy: DeterminacyKind, notes: impl Into<String>) -> Self {
        Self { ok: true, exactness, determinacy, diagnostic_codes: Vec::new(), notes: notes.into() }
    }

    /// 校验失败摘要。
    pub fn failed(notes: impl Into<String>) -> Self {
        Self {
            ok: false,
            exactness: ExactnessKind::Unspecified,
            determinacy: DeterminacyKind::Unspecified,
            diagnostic_codes: Vec::new(),
            notes: notes.into(),
        }
    }
}
