//! 理论层：`Outer` 候选来源；**非** `OuterWorld` 容器。见 [`super::theory`]。

use super::claim::Claim;

/// 尚未接纳的候选关系（来自 solver / verifier 输入；存于 operational `CandidatePool` 或栈上）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OuterCandidate {
    /// 候选 claim（未验证）。
    pub claim: Claim,
}

impl OuterCandidate {
    /// 构造候选。
    pub fn new(claim: Claim) -> Self {
        Self { claim }
    }
}
