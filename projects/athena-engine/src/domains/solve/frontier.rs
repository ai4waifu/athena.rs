//! 可恢复求解前沿（操作性状态，非数学事实）。

use crate::runtime::results::ResultProviderStamp;

/// 机器可读的恢复前沿种类（封闭枚举 · 非用户文案 · 非 M-Graph relation label）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResumeKind {
    /// 通用截断 / 预算切断。
    Cut,
    /// 线性精确求解未完成。
    LinearExact,
    /// 线性数值 / 机器精度路径未完成。
    LinearMachine,
    /// 一元因式分解未完成。
    UnivariateFactor,
}

/// 恢复令牌：待展开分支、未完成量词块、迭代态或 portfolio 状态。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResumeToken {
    /// 前沿种类（操作性，不得参与语义 fingerprint / admission）。
    pub kind: ResumeKind,
    /// 不透明载荷版本。
    pub version: u16,
    /// 产出该前沿的 provider 合同戳（Living `30`：resume 前必须兼容）。
    pub provider: Option<ResultProviderStamp>,
    /// 不透明字节（provider 私有编码；admission 前不得当作证明）。
    pub payload: Vec<u8>,
}

impl ResumeToken {
    /// 空载荷前沿（无 provider 戳 · 仅用于尚未盖戳的 bootstrap 路径）。
    pub fn empty(kind: ResumeKind) -> Self {
        Self { kind, version: 0, provider: None, payload: Vec::new() }
    }

    /// 带当前合同版本戳的空载荷前沿。
    pub fn empty_with_provider(kind: ResumeKind, stamp: ResultProviderStamp) -> Self {
        Self { kind, version: 0, provider: Some(stamp), payload: Vec::new() }
    }

    /// Provider 合同是否允许从此令牌恢复。
    ///
    /// - 令牌无戳：拒绝（禁止用裸 payload 冒充可信恢复）
    /// - 有戳：要求与 `current` 精确兼容
    pub fn accepts_provider(&self, current: ResultProviderStamp) -> bool {
        match self.provider {
            Some(stamp) => stamp.compatible_with(current),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ResumeKind, ResumeToken};
    use crate::runtime::results::{ResultProviderId, ResultProviderStamp};

    #[test]
    fn resume_rejects_missing_provider_stamp() {
        let token = ResumeToken::empty(ResumeKind::Cut);
        assert!(!token.accepts_provider(ResultProviderId::POLYNOMIAL.stamped()));
    }

    #[test]
    fn resume_accepts_matching_provider_stamp() {
        let stamp = ResultProviderId::LINEAR_ALGEBRA.stamped();
        let token = ResumeToken::empty_with_provider(ResumeKind::LinearExact, stamp);
        assert!(token.accepts_provider(stamp));
    }

    #[test]
    fn resume_rejects_stale_provider_version() {
        let stamp = ResultProviderId::NUMBER_THEORY.stamped();
        let token = ResumeToken::empty_with_provider(ResumeKind::Cut, stamp);
        let stale = ResultProviderStamp { id: ResultProviderId::NUMBER_THEORY, version: 0 };
        assert!(!token.accepts_provider(stale));
    }
}
