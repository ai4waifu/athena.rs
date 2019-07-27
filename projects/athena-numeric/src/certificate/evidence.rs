//! 数值证据 arena — 标签与证书留在规范 [`NumericValue`] 之外。

use std::collections::HashMap;

use crate::{certificate::NumericCertificate, number::NumericValue};

/// 已 intern 数值证据的不透明句柄（标签、证书、见证元数据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NumericEvidenceId(pub u32);

/// 存于 [`NumericEvidenceArena`] 的证据载荷，不嵌入 [`NumericValue`]。
#[derive(Debug, PartialEq, Default)]
pub struct NumericEvidenceRecord {
    /// 来源 / 推导标签（结构化 ProofRef 落地前的骨架）。
    pub tags: Vec<String>,
    /// 可选数值证书。
    pub certificate: Option<NumericCertificate>,
}

/// 数值证据 intern 表。规范值本身不含证据。
#[derive(Debug, Default)]
pub struct NumericEvidenceArena {
    records: Vec<NumericEvidenceRecord>,
    tag_index: HashMap<Vec<String>, NumericEvidenceId>,
}

impl NumericEvidenceArena {
    /// 空 arena。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加记录并返回新 id（不消重）。
    pub fn allocate(&mut self, record: NumericEvidenceRecord) -> NumericEvidenceId {
        let id = NumericEvidenceId(self.records.len() as u32);
        self.records.push(record);
        id
    }

    /// 仅按标签列表 intern。标签完全相同时复用已有 id。
    pub fn intern_tags(&mut self, tags: Vec<String>) -> NumericEvidenceId {
        if let Some(&id) = self.tag_index.get(&tags) {
            return id;
        }
        let id = self.allocate(NumericEvidenceRecord { tags: tags.clone(), certificate: None });
        self.tag_index.insert(tags, id);
        id
    }

    /// 解析 id → 记录。
    pub fn resolve(&self, id: NumericEvidenceId) -> Option<&NumericEvidenceRecord> {
        self.records.get(id.0 as usize)
    }

    /// 已存记录数。
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// 规范值加可选外部证据句柄（数学相等只看值）。
#[derive(Debug)]
pub struct NumericBinding {
    value: NumericValue,
    evidence: Option<NumericEvidenceId>,
}

impl NumericBinding {
    /// 无证据的值。
    pub fn new(value: NumericValue) -> Self {
        Self { value, evidence: None }
    }

    /// 带 arena 证据 id 的值。
    pub fn with_evidence(value: NumericValue, evidence: NumericEvidenceId) -> Self {
        Self { value, evidence: Some(evidence) }
    }

    /// 规范数值载荷。
    pub fn value(&self) -> &NumericValue {
        &self.value
    }

    /// 转为规范载荷。
    pub fn into_value(self) -> NumericValue {
        self.value
    }

    /// 若已附加则返回证据句柄。
    pub fn evidence(&self) -> Option<NumericEvidenceId> {
        self.evidence
    }

    /// 附加或替换证据 id。
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
