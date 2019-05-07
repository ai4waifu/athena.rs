//! [`ValueId`] ↔ arena [`TermId`] 绑定。

use std::collections::BTreeMap;

use athena_types::{TermId, ValueId};

/// [`ValueId`] ↔ 存储 [`TermId`] 绑定表。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValueBindingTable {
    value_to_term: BTreeMap<ValueId, TermId>,
    term_to_value: BTreeMap<TermId, ValueId>,
    next: u32,
}

impl ValueBindingTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 为存储项分配值身份。
    pub fn intern_term(&mut self, term: TermId) -> ValueId {
        if let Some(existing) = self.term_to_value.get(&term).copied() {
            return existing;
        }
        let id = ValueId(self.next);
        self.next = self.next.saturating_add(1);
        self.value_to_term.insert(id, term);
        self.term_to_value.insert(term, id);
        id
    }

    /// 查询值对应的存储项。
    pub fn term_of(&self, value: ValueId) -> Option<TermId> {
        self.value_to_term.get(&value).copied()
    }

    /// 查询存储项是否已有值身份。
    pub fn value_of(&self, term: TermId) -> Option<ValueId> {
        self.term_to_value.get(&term).copied()
    }

    /// 已分配数量。
    pub fn len(&self) -> usize {
        self.value_to_term.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.value_to_term.is_empty()
    }
}
