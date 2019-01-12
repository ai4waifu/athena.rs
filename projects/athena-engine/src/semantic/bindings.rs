//! SEM0 语义身份与存储索引的显式绑定（禁止静默互转）。

use std::collections::BTreeMap;

use athena_types::{ExprId, ResultId, TermId, ValueId};

/// [`ExprId`] ↔ arena [`TermId`] 绑定表。
///
/// [`TermId`] 只是存储槽；[`ExprId`] 才是语义表达式身份。同数值载荷不表示同一身份。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExprBindingTable {
    expr_to_term: BTreeMap<ExprId, TermId>,
    term_to_expr: BTreeMap<TermId, ExprId>,
    next: u32,
}

impl ExprBindingTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 为存储项分配新的语义表达式身份。
    pub fn intern_term(&mut self, term: TermId) -> ExprId {
        if let Some(existing) = self.term_to_expr.get(&term).copied() {
            return existing;
        }
        let id = ExprId(self.next);
        self.next = self.next.saturating_add(1);
        self.expr_to_term.insert(id, term);
        self.term_to_expr.insert(term, id);
        id
    }

    /// 查询表达式对应的存储项。
    pub fn term_of(&self, expr: ExprId) -> Option<TermId> {
        self.expr_to_term.get(&expr).copied()
    }

    /// 查询存储项是否已有语义身份。
    pub fn expr_of(&self, term: TermId) -> Option<ExprId> {
        self.term_to_expr.get(&term).copied()
    }

    /// 已分配数量。
    pub fn len(&self) -> usize {
        self.expr_to_term.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.expr_to_term.is_empty()
    }
}

/// [`ValueId`] 注册表（值对象句柄；载荷由领域表解释）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValueIdTable {
    next: u32,
    allocated: BTreeMap<ValueId, ()>,
}

impl ValueIdTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 分配新值身份。
    pub fn alloc(&mut self) -> ValueId {
        let id = ValueId(self.next);
        self.next = self.next.saturating_add(1);
        self.allocated.insert(id, ());
        id
    }

    /// 是否已分配。
    pub fn contains(&self, id: ValueId) -> bool {
        self.allocated.contains_key(&id)
    }

    /// 已分配数量。
    pub fn len(&self) -> usize {
        self.allocated.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.allocated.is_empty()
    }
}

/// [`ResultId`] 注册表（结果容器句柄）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResultIdTable {
    next: u32,
    allocated: BTreeMap<ResultId, ()>,
}

impl ResultIdTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 分配新结果身份。
    pub fn alloc(&mut self) -> ResultId {
        let id = ResultId(self.next);
        self.next = self.next.saturating_add(1);
        self.allocated.insert(id, ());
        id
    }

    /// 是否已分配。
    pub fn contains(&self, id: ResultId) -> bool {
        self.allocated.contains_key(&id)
    }

    /// 已分配数量。
    pub fn len(&self) -> usize {
        self.allocated.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.allocated.is_empty()
    }
}
