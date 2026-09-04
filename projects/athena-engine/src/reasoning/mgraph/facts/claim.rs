//! M-Graph 事实合同：Claim / Scope / Guarantee / Evidence。
//!
//! 已验证事实的唯一语义入口；外围 solver 结果须经 admission gate 转为 [`VerifiedClaim`]。

use athena_types::AssumptionSetId;

use crate::{
    domains::polynomial::{PolynomialCacheKey, PolynomialCacheOp},
    reasoning::mgraph::core::types::SolverId,
};

/// 结论成立范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// 无条件成立。
    Unconditional,
    /// 在假设集下成立。
    UnderAssumptions(AssumptionSetId),
}

/// 可靠性保证层级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Guarantee {
    /// 已证精确。
    ProvenExact,
    /// 条件精确。
    ConditionalExact,
    /// 有界/有证书的近似。
    CertifiedApproximation,
    /// 高概率但未证。
    Probable,
    /// 部分完成（预算截断、未饱和等）。
    Partial,
    /// 下界。
    LowerBound,
    /// 上界。
    UpperBound,
    /// 候选（待进一步验证）。
    Candidate,
    /// 未知。
    Unknown,
}

/// 可验证证据（最小合同；完整 verifier 后续扩展）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// 来自已验证纯 Rust 内核的可重放摘要。
    TrustedKernel {
        /// 产出求解器。
        solver: SolverId,
        /// 人类可读审计摘要。
        summary: String,
    },
}

/// 命题（当前覆盖多项式域；后续扩展等式 / hyper-predicate）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proposition {
    /// 多项式域精确求值结果。
    PolynomialResult {
        /// 缓存操作。
        operation: PolynomialCacheOp,
        /// 请求 canonical 指纹（输入侧）。
        request_fingerprint: u64,
    },
    /// 模同余关系（stable 指纹对 + 模数指纹）。
    Congruence {
        /// 模数 canonical 指纹。
        modulus_fingerprint: u64,
        /// 左操作数 stable 指纹。
        left: u64,
        /// 右操作数 stable 指纹。
        right: u64,
    },
}

/// 未验证候选事实（solver 产出；不得直接进入 exact closure）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// 所述命题。
    pub proposition: Proposition,
    /// 成立范围。
    pub scope: Scope,
    /// 保证层级。
    pub guarantee: Guarantee,
    /// 支撑证据。
    pub evidence: Evidence,
}

/// 经 admission gate 接纳的已验证事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedClaim {
    /// 底层 claim。
    pub claim: Claim,
}

impl VerifiedClaim {
    /// 仅由 [`crate::reasoning::mgraph::admission::gate::EvidenceVerifier`] 构造。
    ///
    /// 禁止任意代码伪造已验证事实（Living `26`）。
    pub(crate) fn from_admission(claim: Claim) -> Self {
        Self { claim }
    }

    /// 是否可进入无条件 exact union-find（当前仅 `Unconditional + ProvenExact`）。
    pub fn admissible_for_exact_union(&self) -> bool {
        matches!((&self.claim.scope, &self.claim.guarantee), (Scope::Unconditional, Guarantee::ProvenExact))
    }
}

/// 从缓存键构造命题。
pub fn proposition_from_cache_key(key: &PolynomialCacheKey) -> Proposition {
    Proposition::PolynomialResult { operation: key.operation, request_fingerprint: key.fingerprint() }
}
