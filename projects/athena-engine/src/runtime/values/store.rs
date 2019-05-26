//! [`ValueId`] → [`RuntimeValue`] 存储（Living `26`：值不是 `TermId` 的第二个名字）。

use std::collections::BTreeMap;

use athena_types::{TermId, ValueId};

/// 运行时值载荷。
///
/// `SymbolicTerm` 只是值的一种情况。禁止用本枚举冒充全部 `TermId` 的别名表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeValue {
    /// 尚未求值为非符号载荷的符号项引用。
    SymbolicTerm(TermId),
    /// 布尔。
    Boolean(bool),
    /// 空值。
    Null,
}

impl RuntimeValue {
    /// 若载荷是符号项，返回其 [`TermId`]。
    pub fn as_symbolic_term(&self) -> Option<TermId> {
        match self {
            Self::SymbolicTerm(id) => Some(*id),
            _ => None,
        }
    }
}

/// [`ValueId`] 所有者：持有真实 [`RuntimeValue`] 载荷。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValueStore {
    next: u32,
    values: BTreeMap<ValueId, RuntimeValue>,
}

impl ValueStore {
    /// 空存储。
    pub fn new() -> Self {
        Self::default()
    }

    /// 插入运行时值并返回新身份。
    pub fn insert(&mut self, value: RuntimeValue) -> ValueId {
        let id = ValueId(self.next);
        self.next = self.next.saturating_add(1);
        self.values.insert(id, value);
        id
    }

    /// 以符号项构造值身份（不是 `TermId`↔`ValueId` 双射表）。
    pub fn insert_symbolic_term(&mut self, term: TermId) -> ValueId {
        self.insert(RuntimeValue::SymbolicTerm(term))
    }

    /// 读取载荷。
    pub fn get(&self, id: ValueId) -> Option<&RuntimeValue> {
        self.values.get(&id)
    }

    /// 是否已分配。
    pub fn contains(&self, id: ValueId) -> bool {
        self.values.contains_key(&id)
    }

    /// 已分配值条数（不是序列长度）。
    pub fn count(&self) -> usize {
        self.values.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
