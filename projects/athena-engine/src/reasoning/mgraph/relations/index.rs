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
        journal::{AdmissionJournal, FactId},
    },
};

/// 单条已接纳关系记录（M-Graph **拥有索引元数据**，**不**复制领域对象本体）。
///
/// Living `31`：**不**实现 [`Clone`]（含 owning [`VerifiedClaim`]）。
#[derive(Debug, PartialEq, Eq)]
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
    /// 从已验证 claim 构造记录（命题映射必须通过谓词注册表 arity 检查）。
    pub fn from_verified(claim: VerifiedClaim) -> Self {
        Self::try_from_verified(claim).expect("verified claim must map to a registered predicate arity")
    }

    /// Fallible construction used by admission / rebuild paths.
    pub fn try_from_verified(claim: VerifiedClaim) -> Result<Self, crate::reasoning::mgraph::admission::AdmissionRejectReason> {
        let scope = scope_to_ref(claim.claim.scope);
        let status = relation_status_from_guarantee(claim.claim.guarantee);
        let (predicate, subjects, theory) = predicate_subjects_theory(&claim.claim.proposition);
        Self::validate_predicate_subjects(predicate, theory, subjects.len())?;
        let provider = provider_from_evidence(&claim.claim.evidence);
        let witness = crate::reasoning::mgraph::facts::witness_ref_from_evidence(&claim.claim.evidence);
        Ok(Self { predicate, subjects, scope, theory, provider, status, witness, verified: claim })
    }

    /// Check predicate registration and subject arity (Living `26`).
    pub fn validate_predicate_subjects(
        predicate: PredicateId,
        theory: TheoryContextId,
        subject_count: usize,
    ) -> Result<(), crate::reasoning::mgraph::admission::AdmissionRejectReason> {
        use crate::reasoning::mgraph::{admission::AdmissionRejectReason, core::predicate_registry};

        let Some(desc) = predicate_registry::descriptor(predicate)
        else {
            return Err(AdmissionRejectReason::MalformedRelation);
        };
        if desc.theory != theory || !desc.subject_arity.contains(&subject_count) {
            return Err(AdmissionRejectReason::MalformedRelation);
        }
        Ok(())
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
        Proposition::CalculusRelation { kind, expression_fingerprint, variable_fingerprint, result_term } => {
            let predicate = match kind {
                crate::reasoning::mgraph::facts::claim::CalculusRelationKind::DerivativeOf => predicates::DERIVATIVE_OF,
                crate::reasoning::mgraph::facts::claim::CalculusRelationKind::IntegralOf => predicates::INTEGRAL_OF,
                crate::reasoning::mgraph::facts::claim::CalculusRelationKind::SeriesExpansion => predicates::SERIES_EXPANSION,
            };
            (
                predicate,
                vec![
                    SemanticRef::Object(ObjectRef::new(TheoryContextId::CALCULUS, *expression_fingerprint)),
                    SemanticRef::Object(ObjectRef::new(TheoryContextId::CALCULUS, *variable_fingerprint)),
                    SemanticRef::Term(*result_term),
                ],
                TheoryContextId::CALCULUS,
            )
        }
        Proposition::TermEquality { left, right } => {
            (predicates::REWRITE_EQUIVALENT, vec![SemanticRef::Term(*left), SemanticRef::Term(*right)], TheoryContextId::REWRITE)
        }
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
///
/// Living `31`：**不**实现 [`Clone`]（含 owning [`RelationRecord`]；可从 journal 重建）。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RelationIndex {
    records: Vec<RelationRecord>,
    by_scope: HashMap<ScopeRef, Vec<RelationRef>>,
    by_predicate: HashMap<(ScopeRef, PredicateId), Vec<RelationRef>>,
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
        self.by_predicate.entry((record.scope, record.predicate)).or_default().push(id);
        self.records.push(record);
        id
    }

    /// 由 [`AdmissionJournal`] 全量重建查询索引（可丢弃后再建）。
    pub fn rebuild_from(journal: &AdmissionJournal) -> Self {
        let mut index = Self::new();
        for claim in journal.claims() {
            index.append(RelationRecord::from_verified(claim.owning_copy()));
        }
        index
    }

    /// 全部记录。
    pub fn records(&self) -> &[RelationRecord] {
        &self.records
    }

    /// 某 scope 下的关系 id。
    pub fn relations_in_scope(&self, scope: ScopeRef) -> &[RelationRef] {
        self.by_scope.get(&scope).map(Vec::as_slice).unwrap_or(&[])
    }

    /// 某 scope + 谓词下的关系 id（二级索引）。
    pub fn relations_with_predicate(&self, scope: ScopeRef, predicate: PredicateId) -> &[RelationRef] {
        self.by_predicate.get(&(scope, predicate)).map(Vec::as_slice).unwrap_or(&[])
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
