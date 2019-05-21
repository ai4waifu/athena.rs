//! 分层定义表与作用域帧（Living `25` L2 · `SymbolId` 键）。
//!
//! - [`DefinitionLayer`]：语句层 Own / Delayed / DownValues（Session 全局为底层）。
//! - [`ScopeFrame`]：`With` / `Module` / `Block` 局部遮蔽（读取优先，不写回全局）。

use std::collections::HashMap;

use athena_types::{ExprId, SymbolId};

/// 语句层定义（`Set` / `SetDelayed` / DownValues）。
#[derive(Debug, Default)]
pub struct DefinitionLayer {
    owns: HashMap<SymbolId, ExprId>,
    delayed: HashMap<SymbolId, ExprId>,
    downs: HashMap<SymbolId, Vec<(ExprId, ExprId)>>,
}

impl DefinitionLayer {
    /// 空层。
    pub fn new() -> Self {
        Self::default()
    }

    /// 写 Own 定义（替换同符号的 Delayed / DownValues，与 legacy 单一定义槽一致）。
    pub fn define_own(&mut self, sym: SymbolId, value: ExprId) {
        self.owns.insert(sym, value);
        self.delayed.remove(&sym);
        self.downs.remove(&sym);
    }

    /// 写 Delayed 定义。
    pub fn define_delayed(&mut self, sym: SymbolId, value: ExprId) {
        self.delayed.insert(sym, value);
        self.owns.remove(&sym);
    }

    /// 追加 DownValue 规则（`f[x_] := rhs`）。
    pub fn define_down_value(&mut self, sym: SymbolId, lhs: ExprId, rhs: ExprId) {
        self.downs.entry(sym).or_default().push((lhs, rhs));
        self.owns.remove(&sym);
    }

    /// 查 Own 值（沿层链由调用方自顶向下查）。
    pub fn own(&self, sym: SymbolId) -> Option<ExprId> {
        self.owns.get(&sym).copied()
    }

    /// 查 Delayed 值。
    pub fn delayed(&self, sym: SymbolId) -> Option<ExprId> {
        self.delayed.get(&sym).copied()
    }

    /// 查 DownValues 规则表。
    pub fn down_values(&self, sym: SymbolId) -> Option<&[(ExprId, ExprId)]> {
        self.downs.get(&sym).map(Vec::as_slice)
    }

    /// 清空层内全部定义。
    pub fn clear(&mut self) {
        self.owns.clear();
        self.delayed.clear();
        self.downs.clear();
    }

    /// 层内是否存在该符号的任何定义。
    pub fn defines(&self, sym: SymbolId) -> bool {
        self.owns.contains_key(&sym) || self.delayed.contains_key(&sym) || self.downs.contains_key(&sym)
    }
}

/// 局部绑定：已初始化值，或未初始化时的唯一化符号（逃逸物化）。
#[derive(Debug, Clone, Copy)]
pub enum LocalBinding {
    /// 已初始化值。
    Own(ExprId),
    /// 未初始化局部的唯一化符号（`name$N`）。
    Unique(ExprId),
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
    pub fn bind(&mut self, sym: SymbolId, binding: LocalBinding) {
        self.locals.insert(sym, binding);
    }

    /// 查局部绑定。
    pub fn lookup(&self, sym: SymbolId) -> Option<LocalBinding> {
        self.locals.get(&sym).copied()
    }
}
