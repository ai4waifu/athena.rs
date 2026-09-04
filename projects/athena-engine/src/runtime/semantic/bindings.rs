//! 值与结果容器身份注册表（Living `25`：`TermId` 已是 arena 原生身份，不再维护二级映射）。

use std::collections::BTreeMap;

use athena_types::{ResultId, ValueId};

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
