//! 证据接纳门控 — 唯一可信接纳边界。
//!
//! `EvidenceVerifier::verify` → [`VerifiedClaim`] → [`SemanticCore::commit`] → [`ExactUnionFind`]。
//!
//! **禁止**「存在证书 / provider 自称 exact ⇒ 接纳」。证书字段必须与命题重放一致。

use crate::{
    domains::polynomial::{PolynomialCacheKey, PolynomialDomainValue, PolynomialResult},
    reasoning::mgraph::{
        core::{state::MGraphState, types::CapabilityProviderId},
        facts::claim::{
            CalculusRelationKind, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, VerifiedClaim,
            proposition_from_cache_key,
        },
        polynomial::{POLYNOMIAL_PROVIDER_ID, PolynomialWitness, witness_from_exact},
    },
};

/// 微积分域 capability provider 身份。
pub const CALCULUS_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(11);

/// 同余关系 capability provider 身份。
pub const CONGRUENCE_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(21);

/// 拒绝接纳原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionRejectReason {
    /// 占位结果。
    Placeholder,
    /// Gröbner 在资源限制内未完成。
    GroebnerIncomplete,
    /// 高概率但未证。
    ProbableResult,
    /// 结果枚举非 Exact。
    NotExact,
    /// 保证层级不足以进入 exact closure。
    InsufficientGuarantee,
    /// 谓词未注册或 subject 元数与 [`crate::reasoning::mgraph::PredicateDescriptor`] 不符。
    MalformedRelation,
    /// 证书与命题不一致，或禁止的夹具/拒绝证书被冒充证明。
    EvidenceMismatch,
}

/// Admission 判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionOutcome {
    /// 已验证并接纳。
    Admitted(VerifiedClaim),
    /// 拒绝（可缓存，不可进 semantic core）。
    Rejected {
        /// 原因。
        reason: AdmissionRejectReason,
        /// 对应的非 exact 保证（用于审计）。
        guarantee: Guarantee,
    },
}

/// Verifier 策略（semantic core 最低保证）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationPolicy {
    /// 进入 semantic core 所需的最低保证。
    pub min_guarantee: Guarantee,
    /// 是否允许 [`EvidenceCertificate::TestHarness`]（仅测试；默认禁止）。
    pub allow_test_harness: bool,
}

impl Default for VerificationPolicy {
    fn default() -> Self {
        Self { min_guarantee: Guarantee::ProvenExact, allow_test_harness: false }
    }
}

impl VerificationPolicy {
    /// 是否接受该保证层级进入 semantic core。
    pub fn accepts(&self, guarantee: Guarantee) -> bool {
        guarantee_rank(guarantee) >= guarantee_rank(self.min_guarantee)
    }

    /// 测试夹具策略：允许 `TestHarness` 证书（不得用于生产路径）。
    pub fn for_test_harness() -> Self {
        Self { min_guarantee: Guarantee::ProvenExact, allow_test_harness: true }
    }
}

/// 可验证证据检查器（trusted kernel 边界）。
pub struct EvidenceVerifier;

impl EvidenceVerifier {
    /// 验证候选 claim 是否可接纳为 [`VerifiedClaim`]。
    ///
    /// 顺序：Probable 拒绝 → 保证门槛 → **证书↔命题重放** → Admitted。
    pub fn verify(claim: &Claim, policy: &VerificationPolicy) -> AdmissionOutcome {
        if claim.guarantee == Guarantee::Probable {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::ProbableResult, guarantee: claim.guarantee };
        }
        if !policy.accepts(claim.guarantee) {
            return AdmissionOutcome::Rejected { reason: reject_reason_for_guarantee(claim.guarantee), guarantee: claim.guarantee };
        }
        if !certificate_replays_proposition(claim, policy) {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, guarantee: claim.guarantee };
        }
        AdmissionOutcome::Admitted(VerifiedClaim::from_admission(claim.clone()))
    }

    /// 验证多项式 solver 产出（Claim 合同判据，非 `PolynomialResult::Exact` 名称）。
    pub fn verify_polynomial(key: &PolynomialCacheKey, result: &PolynomialResult, policy: &VerificationPolicy) -> AdmissionOutcome {
        match result {
            PolynomialResult::Exact { value } => {
                let guarantee = classify_polynomial_guarantee(value);
                let claim = Claim {
                    proposition: proposition_from_cache_key(key),
                    scope: Scope::Unconditional,
                    guarantee,
                    evidence: build_polynomial_evidence(key, value, guarantee),
                };
                Self::verify(&claim, policy)
            }
            PolynomialResult::Unevaluated { .. } => {
                AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, guarantee: Guarantee::Unknown }
            }
        }
    }
}

/// Replay gate: certificate payload must match the claimed proposition.
fn certificate_replays_proposition(claim: &Claim, policy: &VerificationPolicy) -> bool {
    let Evidence::TrustedKernel { certificate, .. } = &claim.evidence;
    match (&claim.proposition, certificate) {
        (
            Proposition::PolynomialResult { operation, request_fingerprint },
            EvidenceCertificate::PolynomialExact { operation: cert_op, request_fingerprint: cert_fp, .. },
        ) => operation == cert_op && request_fingerprint == cert_fp,
        (
            Proposition::Congruence { modulus_fingerprint, left, right },
            EvidenceCertificate::CongruenceExact { modulus_fingerprint: m, left: l, right: r },
        ) => modulus_fingerprint == m && left == l && right == r,
        (
            Proposition::CalculusRelation { kind, expression_fingerprint, variable_fingerprint, result_term },
            EvidenceCertificate::CalculusExact { kind: k, expression_fingerprint: e, variable_fingerprint: v, result_term: t },
        ) => kind == k && expression_fingerprint == e && variable_fingerprint == v && result_term == t,
        (
            Proposition::TermEquality { left, right },
            EvidenceCertificate::StructuralTermEquality { left: l, right: r }
            | EvidenceCertificate::ApplicationCongruence { left: l, right: r }
            | EvidenceCertificate::TypedRewriteReplay { left: l, right: r, .. },
        ) => left == l && right == r,
        (_, EvidenceCertificate::TestHarness) => policy.allow_test_harness,
        (_, EvidenceCertificate::Rejected { .. }) => false,
        _ => false,
    }
}

/// Admission 唯一公开写入入口：`verify` → semantic core（及可选 operational cache）。
pub struct AdmissionGate;

impl AdmissionGate {
    /// 经 [`EvidenceVerifier`] 后写入 semantic core（通用 claim 唯一公开路径）。
    pub fn admit_claim(
        semantic: &mut crate::reasoning::mgraph::admission::semantic::SemanticCore,
        claim: Claim,
        policy: &VerificationPolicy,
    ) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
        match EvidenceVerifier::verify(&claim, policy) {
            AdmissionOutcome::Admitted(vc) => Ok(semantic.commit(vc)),
            AdmissionOutcome::Rejected { reason, .. } => Err(reason),
        }
    }

    /// Admit into [`MGraphState`] and wake matching operational obligations (Living `29`).
    pub fn admit_claim_into_state(
        state: &mut MGraphState,
        claim: Claim,
        policy: &VerificationPolicy,
    ) -> Result<(crate::reasoning::mgraph::facts::FactId, crate::reasoning::mgraph::WakeReport), AdmissionRejectReason> {
        let id = Self::admit_claim(&mut state.semantic, claim, policy)?;
        let Some((predicate, admitted_scope)) = state.semantic.relation(id).map(|record| (record.predicate, record.scope))
        else {
            return Ok((id, crate::reasoning::mgraph::WakeReport::default()));
        };
        let wake = state.operational.obligation_index.wake_matching(admitted_scope, predicate, id, state.semantic.core.scope_index());
        Ok((id, wake))
    }

    /// 接纳多项式结果：operational cache 始终写入，semantic core 仅 verified claim。
    pub fn commit_polynomial(state: &mut MGraphState, key: PolynomialCacheKey, result: PolynomialResult, policy: &VerificationPolicy) {
        let outcome = EvidenceVerifier::verify_polynomial(&key, &result, policy);
        state.operational.result_cache.store_polynomial(key, result, &outcome);
        if let AdmissionOutcome::Admitted(vc) = outcome {
            state.semantic.commit(vc);
        }
    }

    /// 接纳微积分精确表达式关系（无条件 `ProvenExact`）。
    pub fn admit_calculus_relation(
        semantic: &mut crate::reasoning::mgraph::admission::semantic::SemanticCore,
        kind: CalculusRelationKind,
        expression_fingerprint: u64,
        variable_fingerprint: u64,
        result_term: athena_types::TermId,
        policy: &VerificationPolicy,
    ) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
        let claim = Claim {
            proposition: Proposition::CalculusRelation { kind, expression_fingerprint, variable_fingerprint, result_term },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CALCULUS_PROVIDER_ID,
                certificate: EvidenceCertificate::CalculusExact { kind, expression_fingerprint, variable_fingerprint, result_term },
                summary: format!("calculus:{kind:?}:{result_term:?}"),
            },
        };
        Self::admit_claim(semantic, claim, policy)
    }

    /// 接纳无条件 `ProvenExact` 模同余关系（写入 modulus-isolated `CongruenceIndex`）。
    pub fn admit_congruence(
        semantic: &mut crate::reasoning::mgraph::admission::semantic::SemanticCore,
        modulus_fingerprint: u64,
        left: u64,
        right: u64,
        policy: &VerificationPolicy,
    ) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
        let claim = Claim {
            proposition: Proposition::Congruence { modulus_fingerprint, left, right },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CONGRUENCE_PROVIDER_ID,
                certificate: EvidenceCertificate::CongruenceExact { modulus_fingerprint, left, right },
                summary: format!("congruence:{modulus_fingerprint}:{left}:{right}"),
            },
        };
        Self::admit_claim(semantic, claim, policy)
    }
}

/// 对多项式 Exact 值执行 verifier（不写入 semantic core）。
pub fn admit_polynomial_exact(key: &PolynomialCacheKey, value: &PolynomialDomainValue) -> AdmissionOutcome {
    EvidenceVerifier::verify_polynomial(key, &PolynomialResult::Exact { value: value.clone() }, &VerificationPolicy::default())
}

/// 对 [`PolynomialResult`] 执行 verifier（不写入 semantic core）。
pub fn admit_polynomial_result(key: &PolynomialCacheKey, result: &PolynomialResult) -> AdmissionOutcome {
    EvidenceVerifier::verify_polynomial(key, result, &VerificationPolicy::default())
}

/// 是否应写入 semantic core。
pub fn is_admitted(outcome: &AdmissionOutcome) -> bool {
    matches!(outcome, AdmissionOutcome::Admitted(_))
}

fn classify_polynomial_guarantee(value: &PolynomialDomainValue) -> Guarantee {
    match value {
        PolynomialDomainValue::Polynomial(_) => Guarantee::ProvenExact,
        PolynomialDomainValue::GroebnerBasis(v) => {
            if v.is_exact_witness() {
                Guarantee::ProvenExact
            }
            else {
                Guarantee::Partial
            }
        }
        PolynomialDomainValue::UnivariateDivision(v) => {
            if v.remainder.inner.terms().is_empty() {
                Guarantee::ProvenExact
            }
            else {
                Guarantee::Partial
            }
        }
        PolynomialDomainValue::Factorization(v) => {
            if v.is_exact_witness() {
                Guarantee::ProvenExact
            }
            else if v.completeness() == crate::domains::polynomial::PolynomialFactorizationCompleteness::Probable {
                Guarantee::Probable
            }
            else {
                Guarantee::Partial
            }
        }
        PolynomialDomainValue::Placeholder => Guarantee::Unknown,
    }
}

fn build_polynomial_evidence(key: &PolynomialCacheKey, value: &PolynomialDomainValue, guarantee: Guarantee) -> Evidence {
    if guarantee != Guarantee::ProvenExact {
        return Evidence::TrustedKernel {
            provider: POLYNOMIAL_PROVIDER_ID,
            certificate: crate::reasoning::mgraph::facts::claim::EvidenceCertificate::Rejected { guarantee },
            summary: format!("rejected:{guarantee:?}"),
        };
    }
    let witness = witness_from_exact(key, value);
    evidence_from_witness(key, &witness)
}

fn reject_reason_for_guarantee(guarantee: Guarantee) -> AdmissionRejectReason {
    match guarantee {
        Guarantee::Partial => AdmissionRejectReason::GroebnerIncomplete,
        Guarantee::Unknown => AdmissionRejectReason::Placeholder,
        Guarantee::Probable => AdmissionRejectReason::ProbableResult,
        _ => AdmissionRejectReason::InsufficientGuarantee,
    }
}

fn guarantee_rank(g: Guarantee) -> u8 {
    match g {
        Guarantee::Unknown => 0,
        Guarantee::Candidate => 1,
        Guarantee::Probable => 2,
        Guarantee::Partial => 3,
        Guarantee::LowerBound | Guarantee::UpperBound => 4,
        Guarantee::CertifiedApproximation => 5,
        Guarantee::ConditionalExact => 6,
        Guarantee::ProvenExact => 7,
    }
}

fn evidence_from_witness(key: &PolynomialCacheKey, witness: &PolynomialWitness) -> Evidence {
    Evidence::TrustedKernel {
        provider: POLYNOMIAL_PROVIDER_ID,
        certificate: crate::reasoning::mgraph::facts::claim::EvidenceCertificate::PolynomialExact {
            operation: witness.operation,
            request_fingerprint: key.fingerprint(),
            input_hashes: witness.input_hashes.clone(),
            groebner_steps: witness.groebner_steps,
        },
        summary: format!("{}:{}", witness.operation.as_str(), witness.output_summary),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::mgraph::SemanticCore;
    use athena_types::TermId;

    #[test]
    fn admit_congruence_rebuilds_modulus_isolated_index() {
        let mut semantic = SemanticCore::new();
        let policy = VerificationPolicy::default();
        AdmissionGate::admit_congruence(&mut semantic, 7, 10, 20, &policy).expect("mod7");
        AdmissionGate::admit_congruence(&mut semantic, 11, 10, 30, &policy).expect("mod11");
        assert_eq!(semantic.derived.congruence.find(7, 10), semantic.derived.congruence.find(7, 20));
        assert_ne!(semantic.derived.congruence.find(7, 10), semantic.derived.congruence.find(7, 30));
        assert_eq!(semantic.derived.congruence.modulus_count(), 2);
    }

    #[test]
    fn mismatched_calculus_certificate_is_rejected() {
        let claim = Claim {
            proposition: Proposition::CalculusRelation {
                kind: CalculusRelationKind::DerivativeOf,
                expression_fingerprint: 1,
                variable_fingerprint: 2,
                result_term: TermId(3),
            },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CALCULUS_PROVIDER_ID,
                certificate: EvidenceCertificate::CalculusExact {
                    kind: CalculusRelationKind::DerivativeOf,
                    expression_fingerprint: 1,
                    variable_fingerprint: 2,
                    result_term: TermId(99),
                },
                summary: "forged".into(),
            },
        };
        match EvidenceVerifier::verify(&claim, &VerificationPolicy::default()) {
            AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, .. } => {}
            other => panic!("expected EvidenceMismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_harness_rejected_without_policy_flag() {
        let claim = Claim {
            proposition: Proposition::TermEquality { left: TermId(1), right: TermId(1) },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CapabilityProviderId(0),
                certificate: EvidenceCertificate::TestHarness,
                summary: "harness".into(),
            },
        };
        match EvidenceVerifier::verify(&claim, &VerificationPolicy::default()) {
            AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, .. } => {}
            other => panic!("expected EvidenceMismatch, got {other:?}"),
        }
        match EvidenceVerifier::verify(&claim, &VerificationPolicy::for_test_harness()) {
            AdmissionOutcome::Admitted(_) => {}
            other => panic!("expected Admitted under test harness policy, got {other:?}"),
        }
    }

    #[test]
    fn structural_equality_certificate_must_match_terms() {
        let ok = Claim {
            proposition: Proposition::TermEquality { left: TermId(1), right: TermId(2) },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CapabilityProviderId(0),
                certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(1), right: TermId(2) },
                summary: "ok".into(),
            },
        };
        assert!(matches!(EvidenceVerifier::verify(&ok, &VerificationPolicy::default()), AdmissionOutcome::Admitted(_)));
        let bad = Claim {
            evidence: Evidence::TrustedKernel {
                provider: CapabilityProviderId(0),
                certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(1), right: TermId(9) },
                summary: "bad".into(),
            },
            ..ok
        };
        assert!(matches!(
            EvidenceVerifier::verify(&bad, &VerificationPolicy::default()),
            AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, .. }
        ));
    }
}
