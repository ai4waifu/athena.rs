//! Numeric evidence arena — tags and certificates stay outside canonical [`NumericValue`].

use std::collections::HashMap;

use crate::{certificate::NumericCertificate, number::NumericValue};

/// Opaque handle to interned numeric evidence (tags, certificates, witness metadata).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NumericEvidenceId(pub u32);

/// Evidence payload stored in [`NumericEvidenceArena`], not embedded in [`NumericValue`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NumericEvidenceRecord {
    /// Source / derivation tags (skeleton until structured ProofRef).
    pub tags: Vec<String>,
    /// Optional numerical certificate.
    pub certificate: Option<NumericCertificate>,
}

/// Intern table for numeric evidence. Canonical values remain evidence-free.
#[derive(Debug, Default)]
pub struct NumericEvidenceArena {
    records: Vec<NumericEvidenceRecord>,
    tag_index: HashMap<Vec<String>, NumericEvidenceId>,
}

impl NumericEvidenceArena {
    /// Empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a record and return a fresh id (no deduplication).
    pub fn allocate(&mut self, record: NumericEvidenceRecord) -> NumericEvidenceId {
        let id = NumericEvidenceId(self.records.len() as u32);
        self.records.push(record);
        id
    }

    /// Intern by tag list only. Reuses an existing id when tags match exactly.
    pub fn intern_tags(&mut self, tags: Vec<String>) -> NumericEvidenceId {
        if let Some(&id) = self.tag_index.get(&tags) {
            return id;
        }
        let id = self.allocate(NumericEvidenceRecord { tags: tags.clone(), certificate: None });
        self.tag_index.insert(tags, id);
        id
    }

    /// Resolve id → record.
    pub fn resolve(&self, id: NumericEvidenceId) -> Option<&NumericEvidenceRecord> {
        self.records.get(id.0 as usize)
    }

    /// Number of stored records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Canonical value plus optional external evidence handle (math equality uses value only).
#[derive(Debug, Clone)]
pub struct NumericBinding {
    value: NumericValue,
    evidence: Option<NumericEvidenceId>,
}

impl NumericBinding {
    /// Value without evidence.
    pub fn new(value: NumericValue) -> Self {
        Self { value, evidence: None }
    }

    /// Value with an arena evidence id.
    pub fn with_evidence(value: NumericValue, evidence: NumericEvidenceId) -> Self {
        Self { value, evidence: Some(evidence) }
    }

    /// Canonical numeric payload.
    pub fn value(&self) -> &NumericValue {
        &self.value
    }

    /// Into canonical payload.
    pub fn into_value(self) -> NumericValue {
        self.value
    }

    /// Evidence handle if attached.
    pub fn evidence(&self) -> Option<NumericEvidenceId> {
        self.evidence
    }

    /// Attach or replace evidence id.
    pub fn set_evidence(&mut self, evidence: Option<NumericEvidenceId>) {
        self.evidence = evidence;
    }
}

impl PartialEq for NumericBinding {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for NumericBinding {}

/// Deprecated alias for migration from embedded provenance.
#[deprecated(note = "use NumericEvidenceRecord with NumericEvidenceArena")]
pub type NumericProvenance = NumericEvidenceRecord;
