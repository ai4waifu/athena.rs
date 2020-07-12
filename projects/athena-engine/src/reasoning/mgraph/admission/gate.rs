//! 证据接纳门控 — 唯一可信接纳边界。
//!
//! `EvidenceVerifier::verify_in` → [`VerifiedClaim`] → [`SemanticCore::commit`] → [`ExactUnionFind`]。
//!
//! **禁止**「存在证书 / provider 自称 exact ⇒ 接纳」。证书字段必须与命题重放一致，
//! 且关系类证据须在独立上下文中实质成立（例如 `StructuralTermEquality` 经
//! [`TermStore::structural_eq`]）。

use athena_ir::TermStore;
use athena_types::TermId;

use crate::{
    domains::{
        calculus::{CalculusRequest, CalculusResult, CalculusValue, calculus_request_identity, execute_calculus},
        polynomial::{PolynomialCacheKey, PolynomialDomainValue, PolynomialResult, RingTable, verify_groebner_basis},
    },
    reasoning::{
        egraph::applications_congruent,
        mgraph::{
            ExactUnionFind,
            core::{state::MGraphState, types::CapabilityProviderId},
            facts::claim::{
                CalculusRelationKind, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, VerifiedClaim,
                proposition_from_cache_key,
            },
            polynomial::{POLYNOMIAL_PROVIDER_ID, PolynomialWitness, witness_from_exact},
        },
    },
    runtime::session::Session,
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
///
/// **不**实现 [`Clone`]（`Admitted` 变体含 owning [`VerifiedClaim`]）。
#[derive(Debug, PartialEq, Eq)]
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

/// 实质证据检查所需的运行时上下文（非证书字段）。
///
/// `terms` 为可变借用：`TypedRewriteReplay` 重放 `substitute` 可能写入 hash-cons 节点。
#[derive(Debug, Default)]
pub struct VerificationContext<'a> {
    /// 结构相等 / 应用同余 / 类型化改写所需的项存储。
    pub terms: Option<&'a mut TermStore>,
    /// 应用同余所需的已接纳 exact union-find。
    pub exact_uf: Option<&'a ExactUnionFind>,
    /// `TypedRewriteReplay` 所需规则表（缺则该证书实质检查失败）。
    pub typed_rules: Option<&'a crate::reasoning::egraph::TypedRuleSet>,
}

impl<'a> VerificationContext<'a> {
    /// 空上下文（仅字段重放类证书可过，例如多项式指纹 / 同余指纹）。
    pub const fn empty() -> Self {
        Self { terms: None, exact_uf: None, typed_rules: None }
    }

    /// 携带 [`TermStore`]（及可选 ExactUF）。
    pub fn with_terms(terms: &'a mut TermStore, exact_uf: Option<&'a ExactUnionFind>) -> Self {
        Self { terms: Some(terms), exact_uf, typed_rules: None }
    }

    /// 携带 [`TermStore`]、ExactUF 与类型化改写规则表。
    pub fn with_terms_and_typed_rules(
        terms: &'a mut TermStore,
        exact_uf: Option<&'a ExactUnionFind>,
        typed_rules: &'a crate::reasoning::egraph::TypedRuleSet,
    ) -> Self {
        Self { terms: Some(terms), exact_uf, typed_rules: Some(typed_rules) }
    }
}

/// 可验证证据检查器（trusted kernel 边界）。
pub struct EvidenceVerifier;

impl EvidenceVerifier {
    /// 无运行时上下文的验证（`StructuralTermEquality` / `ApplicationCongruence` 将失败）。
    pub fn verify(claim: &Claim, policy: &VerificationPolicy) -> AdmissionOutcome {
        let mut ctx = VerificationContext::empty();
        Self::verify_in(claim, policy, &mut ctx)
    }

    /// 验证候选 claim 是否可接纳为 [`VerifiedClaim`]。
    ///
    /// 顺序：Probable 拒绝 → 保证门槛 → **证书↔命题重放** → **实质证据** → Admitted。
    pub fn verify_in(claim: &Claim, policy: &VerificationPolicy, ctx: &mut VerificationContext<'_>) -> AdmissionOutcome {
        if claim.guarantee == Guarantee::Probable {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::ProbableResult, guarantee: claim.guarantee };
        }
        if !policy.accepts(claim.guarantee) {
            return AdmissionOutcome::Rejected { reason: reject_reason_for_guarantee(claim.guarantee), guarantee: claim.guarantee };
        }
        if !certificate_replays_proposition(claim, policy) {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, guarantee: claim.guarantee };
        }
        if !substantive_evidence_holds(claim, ctx) {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, guarantee: claim.guarantee };
        }
        AdmissionOutcome::Admitted(VerifiedClaim::from_admission(claim.owning_copy()))
    }

    /// 验证多项式 solver 产出（Claim 合同判据，非 `PolynomialResult::Exact` 名称）。
    ///
    /// `rings` 用于独立重放 Gröbner 基：自称 `Verified` 但 critical pairs 不归零时拒绝。
    pub fn verify_polynomial(
        key: &PolynomialCacheKey,
        result: &PolynomialResult,
        policy: &VerificationPolicy,
        rings: Option<&RingTable>,
    ) -> AdmissionOutcome {
        match result {
            PolynomialResult::Exact { value } => {
                if let Some(reason) = reject_unverified_groebner(value, rings) {
                    return AdmissionOutcome::Rejected { reason, guarantee: Guarantee::Partial };
                }
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

    /// 微积分精确关系：优先核对 Session 登记的可信内核结果；无登记时再独立重算。
    ///
    /// 同算法重算只能防字段伪造，**不是**独立数学证明。正常 `execute_calculus` → 准入热路径
    /// 经 [`Session::remember_trusted_calculus`] 免二次计算；外部 / 伪造声称仍走重算。
    /// 通用 [`Self::verify_in`] 对 [`EvidenceCertificate::CalculusExact`] 实质检查恒失败。
    pub fn verify_calculus(
        session: &mut Session,
        request: &CalculusRequest,
        kind: CalculusRelationKind,
        claimed_result: TermId,
        policy: &VerificationPolicy,
    ) -> AdmissionOutcome {
        let Some((expression, variable)) = calculus_expression_variable(request)
        else {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::MalformedRelation, guarantee: Guarantee::Unknown };
        };
        if !calculus_kind_matches_request(kind, request) {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, guarantee: Guarantee::Unknown };
        }
        let expression_fingerprint = u64::from(expression.0);
        let variable_fingerprint = u64::from(variable.0);
        let request_identity = calculus_request_identity(request);
        let matched_trusted = session.take_trusted_calculus_if_matches(request_identity, claimed_result);
        if !matched_trusted {
            let replay = execute_calculus(session, request.owning_copy());
            let CalculusResult::Exact { value: CalculusValue::Expression(replay_term), conditions } = replay
            else {
                return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, guarantee: Guarantee::Unknown };
            };
            if !conditions.is_empty() {
                return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::InsufficientGuarantee, guarantee: Guarantee::ConditionalExact };
            }
            if !session.arena.structural_eq(replay_term, claimed_result) {
                return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, guarantee: Guarantee::Unknown };
            }
        }
        let claim = Claim {
            proposition: Proposition::CalculusRelation {
                kind,
                expression_fingerprint,
                variable_fingerprint,
                request_identity,
                result_term: claimed_result,
            },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CALCULUS_PROVIDER_ID,
                certificate: EvidenceCertificate::CalculusExact {
                    kind,
                    expression_fingerprint,
                    variable_fingerprint,
                    request_identity,
                    result_term: claimed_result,
                },
                summary: format!("calculus:{kind:?}:{request_identity}:{claimed_result:?}"),
            },
        };
        if !policy.accepts(claim.guarantee) {
            return AdmissionOutcome::Rejected { reason: reject_reason_for_guarantee(claim.guarantee), guarantee: claim.guarantee };
        }
        if !certificate_replays_proposition(&claim, policy) {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, guarantee: claim.guarantee };
        }
        AdmissionOutcome::Admitted(VerifiedClaim::from_admission(claim))
    }

    /// 整数同余：`left ≡ right (mod modulus)`，模为 0 或三者字段不一致时拒绝。
    ///
    /// 通用 [`Self::verify_in`] 对 [`EvidenceCertificate::CongruenceExact`] 实质检查恒失败。
    /// `modulus_fingerprint` / `left` / `right` 在此入口解释为非负整数操作数（非任意哈希）。
    pub fn verify_congruence(
        modulus: u64,
        left: u64,
        right: u64,
        policy: &VerificationPolicy,
    ) -> AdmissionOutcome {
        if modulus == 0 {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::MalformedRelation, guarantee: Guarantee::Unknown };
        }
        let m = i128::from(modulus);
        let congruent = (i128::from(left) - i128::from(right)).rem_euclid(m) == 0;
        if !congruent {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, guarantee: Guarantee::Unknown };
        }
        let claim = Claim {
            proposition: Proposition::Congruence { modulus_fingerprint: modulus, left, right },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CONGRUENCE_PROVIDER_ID,
                certificate: EvidenceCertificate::CongruenceExact { modulus_fingerprint: modulus, left, right },
                summary: format!("congruence:{modulus}:{left}:{right}"),
            },
        };
        if !policy.accepts(claim.guarantee) {
            return AdmissionOutcome::Rejected { reason: reject_reason_for_guarantee(claim.guarantee), guarantee: claim.guarantee };
        }
        if !certificate_replays_proposition(&claim, policy) {
            return AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, guarantee: claim.guarantee };
        }
        AdmissionOutcome::Admitted(VerifiedClaim::from_admission(claim))
    }
}

fn calculus_expression_variable(request: &CalculusRequest) -> Option<(TermId, athena_types::SymbolId)> {
    match request {
        CalculusRequest::Derivative { expression, variable, .. }
        | CalculusRequest::Integral { expression, variable, .. }
        | CalculusRequest::DefiniteIntegral { expression, variable, .. }
        | CalculusRequest::Series { expression, variable, .. }
        | CalculusRequest::Laurent { expression, variable, .. }
        | CalculusRequest::Asymptotic { expression, variable, .. } => Some((*expression, *variable)),
        _ => None,
    }
}

fn calculus_kind_matches_request(kind: CalculusRelationKind, request: &CalculusRequest) -> bool {
    match (kind, request) {
        (CalculusRelationKind::DerivativeOf, CalculusRequest::Derivative { .. }) => true,
        (CalculusRelationKind::IntegralOf, CalculusRequest::Integral { .. } | CalculusRequest::DefiniteIntegral { .. }) => true,
        (
            CalculusRelationKind::SeriesExpansion,
            CalculusRequest::Series { .. } | CalculusRequest::Laurent { .. } | CalculusRequest::Asymptotic { .. },
        ) => true,
        _ => false,
    }
}

/// 回放门控：证书载荷必须与所声明命题一致。
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
            Proposition::CalculusRelation {
                kind,
                expression_fingerprint,
                variable_fingerprint,
                request_identity,
                result_term,
            },
            EvidenceCertificate::CalculusExact {
                kind: k,
                expression_fingerprint: e,
                variable_fingerprint: v,
                request_identity: rid,
                result_term: t,
            },
        ) => kind == k && expression_fingerprint == e && variable_fingerprint == v && request_identity == rid && result_term == t,
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

/// 实质证据：字段一致之外，关系类证书必须在上下文中独立成立。
fn substantive_evidence_holds(claim: &Claim, ctx: &mut VerificationContext<'_>) -> bool {
    let Evidence::TrustedKernel { certificate, .. } = &claim.evidence;
    match certificate {
        EvidenceCertificate::StructuralTermEquality { left, right } => {
            let Some(store) = ctx.terms.as_ref()
            else {
                return false;
            };
            store.structural_eq(*left, *right)
        }
        EvidenceCertificate::ApplicationCongruence { left, right } => match (ctx.terms.as_ref(), ctx.exact_uf) {
            (Some(store), Some(uf)) => applications_congruent(store, uf, *left, *right),
            _ => false,
        },
        EvidenceCertificate::TypedRewriteReplay { rule, left, right } => match (ctx.terms.as_mut(), ctx.typed_rules) {
            (Some(store), Some(rules)) => crate::reasoning::egraph::typed_rewrite_holds(store, rules, *rule, *left, *right),
            _ => false,
        },
        EvidenceCertificate::CalculusExact { .. } => {
            // 字段一致 ≠ 微积分成立。须经 [`EvidenceVerifier::verify_calculus`]（可信登记或重算）。
            false
        }
        EvidenceCertificate::CongruenceExact { .. } => {
            // 字段一致 ≠ 同余成立。须经 [`EvidenceVerifier::verify_congruence`]（整数模运算）。
            false
        }
        EvidenceCertificate::PolynomialExact { .. } | EvidenceCertificate::TestHarness | EvidenceCertificate::Rejected { .. } => true,
    }
}

/// Admission 唯一公开写入入口：`verify` → semantic core（及可选 operational cache）。
pub struct AdmissionGate;

impl AdmissionGate {
    /// 经 [`EvidenceVerifier`] 后写入 semantic core（唯一公开写入路径）。
    ///
    /// `typed_rules`：仅 `TypedRewriteReplay` 需要；其它证书传 `None`。
    /// 缺规则表时 `TypedRewriteReplay` 实质检查失败（不得字段伪造准入）。
    pub fn admit_claim(
        terms: &mut TermStore,
        semantic: &mut crate::reasoning::mgraph::admission::semantic::SemanticCore,
        claim: Claim,
        policy: &VerificationPolicy,
        typed_rules: Option<&crate::reasoning::egraph::TypedRuleSet>,
    ) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
        let outcome = {
            let uf = &semantic.derived.exact_uf;
            let mut ctx = match typed_rules {
                Some(rules) => VerificationContext::with_terms_and_typed_rules(terms, Some(uf), rules),
                None => VerificationContext::with_terms(terms, Some(uf)),
            };
            EvidenceVerifier::verify_in(&claim, policy, &mut ctx)
        };
        match outcome {
            AdmissionOutcome::Admitted(vc) => Ok(semantic.commit(vc)),
            AdmissionOutcome::Rejected { reason, .. } => Err(reason),
        }
    }

    /// Admit claim and record proof premises（可重放证明依赖）。
    ///
    /// Dependency 登记失败时事实仍保留；调用方须处理诊断（bootstrap：不得静默丢依赖）。
    pub fn admit_claim_with_premises(
        terms: &mut TermStore,
        semantic: &mut crate::reasoning::mgraph::admission::semantic::SemanticCore,
        claim: Claim,
        policy: &VerificationPolicy,
        premises: &[crate::reasoning::mgraph::facts::FactId],
        typed_rules: Option<&crate::reasoning::egraph::TypedRuleSet>,
    ) -> Result<(crate::reasoning::mgraph::facts::FactId, Result<(), athena_types::Diagnostic>), AdmissionRejectReason> {
        let id = Self::admit_claim(terms, semantic, claim, policy, typed_rules)?;
        let dep = semantic.record_proof_dependencies(id, premises);
        Ok((id, dep))
    }

    /// 接纳进 [`MGraphState`]，并唤醒匹配的操作义务。
    pub fn admit_claim_into_state(
        terms: &mut TermStore,
        state: &mut MGraphState,
        claim: Claim,
        policy: &VerificationPolicy,
        typed_rules: Option<&crate::reasoning::egraph::TypedRuleSet>,
    ) -> Result<(crate::reasoning::mgraph::facts::FactId, crate::reasoning::mgraph::WakeReport), AdmissionRejectReason> {
        let id = Self::admit_claim(terms, &mut state.semantic, claim, policy, typed_rules)?;
        let Some((predicate, admitted_scope)) = state.semantic.relation(id).map(|record| (record.predicate, record.scope))
        else {
            return Ok((id, crate::reasoning::mgraph::WakeReport::default()));
        };
        let wake = state.operational.obligation_index.wake_matching(admitted_scope, predicate, id, state.semantic.core.scope_index());
        Ok((id, wake))
    }

    /// 接纳多项式结果：operational cache 始终写入，semantic core 仅 verified claim。
    pub fn commit_polynomial(
        state: &mut MGraphState,
        key: PolynomialCacheKey,
        result: PolynomialResult,
        policy: &VerificationPolicy,
        rings: Option<&RingTable>,
    ) {
        let outcome = EvidenceVerifier::verify_polynomial(&key, &result, policy, rings);
        state.operational.result_cache.store_polynomial(key, result, &outcome);
        if let AdmissionOutcome::Admitted(vc) = outcome {
            state.semantic.commit(vc);
        }
    }

    /// 接纳微积分精确表达式关系（无条件 `ProvenExact`）。
    ///
    /// 经可信内核结果核对或独立重算后写入。正常热路径免二次 `execute_calculus`。
    pub fn admit_calculus_relation(
        session: &mut Session,
        request: &CalculusRequest,
        kind: CalculusRelationKind,
        result_term: athena_types::TermId,
        policy: &VerificationPolicy,
    ) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
        match EvidenceVerifier::verify_calculus(session, request, kind, result_term, policy) {
            AdmissionOutcome::Admitted(vc) => Ok(session.mgraph.semantic.commit(vc)),
            AdmissionOutcome::Rejected { reason, .. } => Err(reason),
        }
    }

    /// 接纳无条件 `ProvenExact` 整数同余关系（写入 modulus-isolated `CongruenceIndex`）。
    ///
    /// 经 [`EvidenceVerifier::verify_congruence`]：要求 `left ≡ right (mod modulus)`，禁止字段-only 准入。
    pub fn admit_congruence(
        _terms: &mut TermStore,
        semantic: &mut crate::reasoning::mgraph::admission::semantic::SemanticCore,
        modulus_fingerprint: u64,
        left: u64,
        right: u64,
        policy: &VerificationPolicy,
    ) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
        match EvidenceVerifier::verify_congruence(modulus_fingerprint, left, right, policy) {
            AdmissionOutcome::Admitted(vc) => Ok(semantic.commit(vc)),
            AdmissionOutcome::Rejected { reason, .. } => Err(reason),
        }
    }
}

/// 对多项式 Exact 值执行 verifier（不写入 semantic core）。
pub fn admit_polynomial_exact(key: &PolynomialCacheKey, value: &PolynomialDomainValue) -> AdmissionOutcome {
    EvidenceVerifier::verify_polynomial(key, &PolynomialResult::Exact { value: value.owning_copy() }, &VerificationPolicy::default(), None)
}

/// 对 [`PolynomialResult`] 执行 verifier（不写入 semantic core）。
pub fn admit_polynomial_result(key: &PolynomialCacheKey, result: &PolynomialResult) -> AdmissionOutcome {
    EvidenceVerifier::verify_polynomial(key, result, &VerificationPolicy::default(), None)
}

/// 带环表的多项式 verifier（Gröbner 独立重放）。
pub fn admit_polynomial_result_with_rings(key: &PolynomialCacheKey, result: &PolynomialResult, rings: &RingTable) -> AdmissionOutcome {
    EvidenceVerifier::verify_polynomial(key, result, &VerificationPolicy::default(), Some(rings))
}

/// 自称已验证的 Gröbner 基须经独立 critical-pair 重放；无环表则拒绝 exact admission。
fn reject_unverified_groebner(value: &PolynomialDomainValue, rings: Option<&RingTable>) -> Option<AdmissionRejectReason> {
    let PolynomialDomainValue::GroebnerBasis(v) = value
    else {
        return None;
    };
    if !v.is_exact_witness() {
        return None;
    }
    let Some(rings) = rings
    else {
        return Some(AdmissionRejectReason::EvidenceMismatch);
    };
    match verify_groebner_basis(&v.basis, rings) {
        Ok(report) if report.all_s_pairs_reduce_to_zero => None,
        Ok(_) => Some(AdmissionRejectReason::EvidenceMismatch),
        Err(_) => Some(AdmissionRejectReason::EvidenceMismatch),
    }
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
        // 模图像是 𝔽ₚ 上的候选。它们不得在 ℤ/ℚ 上成为 ProvenExact。
        PolynomialDomainValue::ModularImage(_) => Guarantee::Partial,
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
