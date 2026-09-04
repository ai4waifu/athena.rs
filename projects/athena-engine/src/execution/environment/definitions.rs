//! 分层定义表与作用域帧（Living `25` L2 · `SymbolId` 键）。
//!
//! - [`DefinitionLayer`]：语句层 Own / Delayed / DownValues（Session 全局为底层）。
//! - [`ScopeFrame`]：`With` / `Module` / `Block` 局部遮蔽（读取优先，不写回全局）。

use std::collections::HashMap;

use athena_types::{SymbolId, TermId};

/// 语句层定义（`Set` / `SetDelayed` / DownValues）。
#[derive(Debug, Default)]
pub struct DefinitionLayer {
    owns: HashMap<SymbolId, TermId>,
    delayed: HashMap<SymbolId, TermId>,
    downs: HashMap<SymbolId, Vec<(TermId, TermId)>>,
}

impl DefinitionLayer {
    /// 空层。
    pub fn new() -> Self {
        Self::default()
    }

    /// 写 Own 定义（替换同符号的 Delayed / DownValues，与 legacy 单一定义槽一致）。
    pub fn define_own(&mut self, symbol: SymbolId, value: TermId) {
        self.owns.insert(symbol, value);
        self.delayed.remove(&symbol);
        self.downs.remove(&symbol);
    }

    /// 写 Delayed 定义。
    pub fn define_delayed(&mut self, symbol: SymbolId, value: TermId) {
        self.delayed.insert(symbol, value);
        self.owns.remove(&symbol);
    }

    /// 追加 DownValue 规则（`f[x_] := rhs`）。
    pub fn define_down_value(&mut self, symbol: SymbolId, lhs: TermId, rhs: TermId) {
        self.downs.entry(symbol).or_default().push((lhs, rhs));
        self.owns.remove(&symbol);
    }

    /// 查 Own 值（沿层链由调用方自顶向下查）。
    pub fn own(&self, symbol: SymbolId) -> Option<TermId> {
        self.owns.get(&symbol).copied()
    }

    /// 查 Delayed 值。
    pub fn delayed(&self, symbol: SymbolId) -> Option<TermId> {
        self.delayed.get(&symbol).copied()
    }

    /// 查 DownValues 规则表。
    pub fn down_values(&self, symbol: SymbolId) -> Option<&[(TermId, TermId)]> {
        self.downs.get(&symbol).map(Vec::as_slice)
    }

    /// 清空层内全部定义。
    pub fn clear(&mut self) {
        self.owns.clear();
        self.delayed.clear();
        self.downs.clear();
    }

    /// 层内是否存在该符号的任何定义。
    pub fn defines(&self, symbol: SymbolId) -> bool {
        self.owns.contains_key(&symbol) || self.delayed.contains_key(&symbol) || self.downs.contains_key(&symbol)
    }
}

/// 局部绑定：已初始化值，或未初始化时的唯一化符号（逃逸物化）。
#[derive(Debug, Clone, Copy)]
pub enum LocalBinding {
    /// 已初始化值。
    Own(TermId),
    /// 未初始化局部的唯一化符号（`name$N`）。
    Unique(TermId),
}

/// 作用域帧：`With` / `Module` / `Block` 局部符号 → 绑定。
#[derive(Debug, Clone, Default)]
pub struct ScopeFrame {
    locals: HashMap<SymbolId, LocalBinding>,
}

impl ScopeFrame {
    /// 空帧。
    pub fn new() -> Self {
        Self::default()
    }

    /// 绑定局部。
    pub fn bind(&mut self, symbol: SymbolId, binding: LocalBinding) {
        self.locals.insert(symbol, binding);
    }

    /// 查局部绑定。
    pub fn lookup(&self, symbol: SymbolId) -> Option<LocalBinding> {
        self.locals.get(&symbol).copied()
    }
}
