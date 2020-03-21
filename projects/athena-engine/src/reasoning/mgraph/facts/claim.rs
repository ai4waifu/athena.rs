//! M-Graph 事实合同：Claim / Scope / Guarantee / Evidence。
//!
//! 已验证事实的唯一语义入口；外围 solver 结果须经 admission gate 转为 [`VerifiedClaim`]。

use athena_types::{AssumptionSetId, TermId};

use crate::{
    domains::polynomial::{PolynomialCacheKey, PolynomialCacheOp},
    reasoning::mgraph::core::types::CapabilityProviderId,
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

/// 可验证证据证书（机器可读字段；`summary` 仅展示）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub enum EvidenceCertificate {
    /// 多项式精确运算证书。
    PolynomialExact {
        /// 缓存操作。
        operation: PolynomialCacheOp,
        /// 请求指纹。
        request_fingerprint: u64,
        /// 输入哈希序列。
        input_hashes: Vec<u64>,
        /// Gröbner S-pair 步数（若适用）。
        groebner_steps: Option<u32>,
    },
    /// 拒绝接纳时的审计占位（不得冒充证明）。
    Rejected {
        /// 当时的保证层级。
        guarantee: Guarantee,
    },
    /// 仅测试夹具（禁止生产路径）。
    TestHarness,
    /// 微积分精确关系证书（表达式结果项）。
    CalculusExact {
        /// 关系种类。
        kind: CalculusRelationKind,
        /// 输入表达式指纹。
        expression_fingerprint: u64,
        /// 变量指纹。
        variable_fingerprint: u64,
        /// 结果项。
        result_term: TermId,
    },
    /// TermStore 结构相等检查通过（E-Graph 候选升级）。
    StructuralTermEquality {
        /// 左项。
        left: TermId,
        /// 右项。
        right: TermId,
    },
    /// 模同余精确关系证书（stable 指纹）。
    CongruenceExact {
        /// 模数 canonical 指纹。
        modulus_fingerprint: u64,
        /// 左操作数 stable 指纹。
        left: u64,
        /// 右操作数 stable 指纹。
        right: u64,
    },
    /// 应用同余：头相同且各参数在 ExactUF 下等价。
    ApplicationCongruence {
        /// 左应用项。
        left: TermId,
        /// 右应用项。
        right: TermId,
    },
    /// Typed [`TermPattern`] 改写重放通过（match + substitute）。
    TypedRewriteReplay {
        /// 规则身份。
        rule: athena_rewriter::RewriteRuleId,
        /// 匹配主体。
        left: TermId,
        /// 重放产出（与候选右侧结构相等）。
        right: TermId,
    },
}

impl EvidenceCertificate {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::PolynomialExact { operation, request_fingerprint, input_hashes, groebner_steps } => Self::PolynomialExact {
                operation: *operation,
                request_fingerprint: *request_fingerprint,
                input_hashes: input_hashes.clone(),
                groebner_steps: *groebner_steps,
            },
            Self::Rejected { guarantee } => Self::Rejected { guarantee: *guarantee },
            Self::TestHarness => Self::TestHarness,
            Self::CalculusExact { kind, expression_fingerprint, variable_fingerprint, result_term } => Self::CalculusExact {
                kind: *kind,
                expression_fingerprint: *expression_fingerprint,
                variable_fingerprint: *variable_fingerprint,
                result_term: *result_term,
            },
            Self::StructuralTermEquality { left, right } => Self::StructuralTermEquality { left: *left, right: *right },
            Self::CongruenceExact { modulus_fingerprint, left, right } => {
                Self::CongruenceExact { modulus_fingerprint: *modulus_fingerprint, left: *left, right: *right }
            }
            Self::ApplicationCongruence { left, right } => Self::ApplicationCongruence { left: *left, right: *right },
            Self::TypedRewriteReplay { rule, left, right } => Self::TypedRewriteReplay { rule: *rule, left: *left, right: *right },
        }
    }
}

/// 可验证证据（最小合同；完整 EvidenceStore 后续扩展）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub enum Evidence {
    /// 来自已验证纯 Rust 内核。
    TrustedKernel {
        /// 产出 capability provider。
        provider: CapabilityProviderId,
        /// 机器可读证书。
        certificate: EvidenceCertificate,
        /// 人类可读审计摘要（仅展示，不得单独充当证明本体）。
        summary: String,
    },
}

impl Evidence {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::TrustedKernel { provider, certificate, summary } => Self::TrustedKernel {
                provider: *provider,
                certificate: certificate.owning_copy(),
                summary: summary.clone(),
            },
        }
    }
}

/// 微积分已接纳关系种类（闭合枚举，非前端名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalculusRelationKind {
    /// `DerivativeOf`。
    DerivativeOf,
    /// `IntegralOf`。
    IntegralOf,
    /// `SeriesExpansion`。
    SeriesExpansion,
}

/// 命题（当前覆盖多项式 / 微积分 / 同余；后续扩展等式 / hyper-predicate）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// 微积分精确关系（结果为表达式项）。
    CalculusRelation {
        /// 关系种类。
        kind: CalculusRelationKind,
        /// 输入表达式指纹。
        expression_fingerprint: u64,
        /// 变量指纹。
        variable_fingerprint: u64,
        /// 结果项（payload 仍在 TermStore）。
        result_term: TermId,
    },
    /// 两项语义等价（经 E-Graph 候选 + Verifier / structural 检查后可接纳）。
    TermEquality {
        /// 左项。
        left: TermId,
        /// 右项。
        right: TermId,
    },
}

/// 未验证候选事实（solver 产出；不得直接进入 exact closure）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
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

impl Claim {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            proposition: self.proposition,
            scope: self.scope,
            guarantee: self.guarantee,
            evidence: self.evidence.owning_copy(),
        }
    }
}

/// 经 admission gate 接纳的已验证事实。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
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

    /// Owning 复制（Living `31`）。复制嵌套 [`Claim`]。
    pub fn owning_copy(&self) -> Self {
        Self { claim: self.claim.owning_copy() }
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
