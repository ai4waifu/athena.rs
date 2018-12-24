//! Binding-aware 变量身份。

use std::collections::BTreeMap;

use athena_types::{SymbolId, TermId};

/// Session / arena 内的绑定槽位（与裸 [`SymbolId`] 区分）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BindingId(pub u32);

/// 带绑定上下文的符号。
///
/// Solve 变量身份必须是 binding-aware，禁止用无序 `Vec<String>` 冒充。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BoundSymbol {
    /// 稳定符号 id。
    pub symbol: SymbolId,
    /// 绑定槽；`None` 表示当前 Session 顶层自由符号。
    pub binding: Option<BindingId>,
}

impl BoundSymbol {
    /// 顶层自由符号。
    pub fn free(symbol: SymbolId) -> Self {
        Self { symbol, binding: None }
    }

    /// 显式绑定槽。
    pub fn bound(symbol: SymbolId, binding: BindingId) -> Self {
        Self { symbol, binding: Some(binding) }
    }
}

/// 解分支上的变量绑定：[`BoundSymbol`] → 解表达式 [`TermId`]。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BindingMap {
    /// 有序绑定表（稳定比较）。
    pub entries: BTreeMap<BoundSymbol, TermId>,
}

impl BindingMap {
    /// 空绑定。
    pub fn empty() -> Self {
        Self::default()
    }

    /// 插入或覆盖一个绑定。
    pub fn insert(&mut self, symbol: BoundSymbol, value: TermId) {
        self.entries.insert(symbol, value);
    }

    /// 查询。
    pub fn get(&self, symbol: &BoundSymbol) -> Option<TermId> {
        self.entries.get(symbol).copied()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
