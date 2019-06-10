//! 已接纳关系索引（实现层 `I_rel` / 纤维化 `𝓕(w)` 的存储形态）。
//! 见 [`crate::reasoning::mgraph::relations::theory`] §实现签名。

use std::collections::HashMap;

use crate::reasoning::mgraph::{
    core::{
        refs::{
            ObjectRef, PredicateId, RelationRef, RelationStatus, ScopeRef, SemanticRef, TheoryContextId, WitnessRef, predicates, scope_to_ref,
        },
        types::CapabilityProviderId,
    },
    facts::{
        claim::{Evidence, Guarantee, Proposition, VerifiedClaim},
        journal::FactId,
    },
};

/// 单条已接纳关系记录（M-Graph **拥有索引元数据**，**不**复制领域对象本体）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationRecord {
    /// 稳定语义谓词（禁止 `String` 标签）。
    pub predicate: PredicateId,
    /// 关系主体引用（不拥有 payload）。
    pub subjects: Vec<SemanticRef>,
    /// 所属 scope。
    pub scope: ScopeRef,
    /// 理论上下文。
    pub theory: TheoryContextId,
    /// 产出该关系的 capability provider。
    pub provider: Option<CapabilityProviderId>,
    /// 接纳状态。
    pub status: RelationStatus,
    /// 证据引用（可为 `None`，证据仍在 claim 内）。
    pub witness: Option<WitnessRef>,
    /// 已验证 claim 载荷（领域 [`Proposition`] 由 claim 解释；非第二套 IR 目标态）。
    pub verified: VerifiedClaim,
}

impl RelationRecord {
    /// 从已验证 claim 构造记录。
    pub fn from_verified(claim: VerifiedClaim) -> Self {
        let scope = scope_to_ref(claim.claim.scope);
        let status = relation_status_from_guarantee(claim.claim.guarantee);
        let (predicate, subjects, theory) = predicate_subjects_theory(&claim.claim.proposition);
        let provider = provider_from_evidence(&claim.claim.evidence);
        Self { predicate, subjects, scope, theory, provider, status, witness: None, verified: claim }
    }
}

fn provider_from_evidence(evidence: &Evidence) -> Option<CapabilityProviderId> {
    match evidence {
        Evidence::TrustedKernel { provider, .. } => Some(*provider),
    }
}

fn predicate_subjects_theory(proposition: &Proposition) -> (PredicateId, Vec<SemanticRef>, TheoryContextId) {
    match proposition {
        Proposition::PolynomialResult { request_fingerprint, .. } => (
            predicates::POLYNOMIAL_RESULT,
            vec![SemanticRef::Object(ObjectRef::new(TheoryContextId::POLYNOMIAL, *request_fingerprint))],
            TheoryContextId::POLYNOMIAL,
        ),
        Proposition::Congruence { modulus_fingerprint, left, right } => (
            predicates::CONGRUENCE,
            vec![
                SemanticRef::Object(ObjectRef::new(TheoryContextId::CONGRUENCE, *left)),
                SemanticRef::Object(ObjectRef::new(TheoryContextId::CONGRUENCE, *right)),
                SemanticRef::Object(ObjectRef::new(TheoryContextId::CONGRUENCE, *modulus_fingerprint)),
            ],
            TheoryContextId::CONGRUENCE,
        ),
    }
}

fn relation_status_from_guarantee(g: Guarantee) -> RelationStatus {
    match g {
        Guarantee::ProvenExact => RelationStatus::Accepted,
        Guarantee::ConditionalExact | Guarantee::CertifiedApproximation => RelationStatus::Conditional,
        Guarantee::Probable
        | Guarantee::Partial
        | Guarantee::LowerBound
        | Guarantee::UpperBound
        | Guarantee::Candidate
        | Guarantee::Unknown => RelationStatus::Conditional,
    }
}

/// `ScopeRef` → 该 scope 下 [`RelationRef`] 列表。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelationIndex {
    records: Vec<RelationRecord>,
    by_scope: HashMap<ScopeRef, Vec<RelationRef>>,
}

impl RelationIndex {
    /// 空索引。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加已接纳关系（单调；id = 下标）。仅 [`crate::reasoning::mgraph::core::MGraphCore::admit`] 可写。
    pub(crate) fn append(&mut self, record: RelationRecord) -> RelationRef {
        let id = FactId(self.records.len() as u64);
        self.by_scope.entry(record.scope).or_default().push(id);
        self.records.push(record);
        id
    }

    /// 全部记录。
    pub fn records(&self) -> &[RelationRecord] {
        &self.records
    }

    /// 某 scope 下的关系 id。
    pub fn relations_in_scope(&self, scope: ScopeRef) -> &[RelationRef] {
        self.by_scope.get(&scope).map(Vec::as_slice).unwrap_or(&[])
    }

    /// 按 id 查记录。
    pub fn get(&self, id: RelationRef) -> Option<&RelationRecord> {
        self.records.get(id.0 as usize)
    }

    /// 已接纳关系条数。
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
