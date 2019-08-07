//! 分层定义表与作用域帧（Living `25` / `27` · `SymbolId` 键）。
//!
//! - [`DefinitionLayer`]：语句层立即绑定 / 残余绑定 / 规则分派（Session 全局为底层）。
//! - [`ScopeFrame`]：`LocalScope` / `LexicalScope` / `DynamicScope` 局部遮蔽（读取优先，不写回全局）。

use std::collections::HashMap;

use athena_types::{SymbolId, TermId};

/// 语句层定义（立即绑定 / 残余绑定 / 规则分派）。
#[derive(Debug, Default)]
pub struct DefinitionLayer {
    bindings: HashMap<SymbolId, TermId>,
    residual_bindings: HashMap<SymbolId, TermId>,
    dispatch_rules: HashMap<SymbolId, Vec<(TermId, TermId)>>,
}

impl DefinitionLayer {
    /// 空层。
    pub fn new() -> Self {
        Self::default()
    }

    /// 写入立即求值绑定（替换同符号的残余绑定与分派规则）。
    pub fn write_binding(&mut self, symbol: SymbolId, value: TermId) {
        self.bindings.insert(symbol, value);
        self.residual_bindings.remove(&symbol);
        self.dispatch_rules.remove(&symbol);
    }

    /// 写入残余项绑定（读取 / 应用时再求值）。
    pub fn write_residual_binding(&mut self, symbol: SymbolId, value: TermId) {
        self.residual_bindings.insert(symbol, value);
        self.bindings.remove(&symbol);
    }

    /// 追加规则分派条目（pattern → replacement）。
    pub fn register_rule(&mut self, symbol: SymbolId, pattern: TermId, replacement: TermId) {
        self.dispatch_rules.entry(symbol).or_default().push((pattern, replacement));
        self.bindings.remove(&symbol);
    }

    /// 查立即绑定。
    pub fn binding(&self, symbol: SymbolId) -> Option<TermId> {
        self.bindings.get(&symbol).copied()
    }

    /// 查残余绑定。
    pub fn residual_binding(&self, symbol: SymbolId) -> Option<TermId> {
        self.residual_bindings.get(&symbol).copied()
    }

    /// 查规则分派表。
    pub fn dispatch_rules(&self, symbol: SymbolId) -> Option<&[(TermId, TermId)]> {
        self.dispatch_rules.get(&symbol).map(Vec::as_slice)
    }

    /// 清除该符号的全部绑定与分派规则。
    pub fn clear_symbol(&mut self, symbol: SymbolId) {
        self.bindings.remove(&symbol);
        self.residual_bindings.remove(&symbol);
        self.dispatch_rules.remove(&symbol);
    }

    /// 清空层内全部定义。
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.residual_bindings.clear();
        self.dispatch_rules.clear();
    }

    /// 层内是否存在该符号的任何定义。
    pub fn defines(&self, symbol: SymbolId) -> bool {
        self.bindings.contains_key(&symbol)
            || self.residual_bindings.contains_key(&symbol)
            || self.dispatch_rules.contains_key(&symbol)
    }
}

/// 局部绑定：已初始化值，或未初始化时的唯一化符号（逃逸物化）。
#[derive(Debug, Clone, Copy)]
pub enum LocalBinding {
    /// 已初始化值。
    Value(TermId),
    /// 未初始化局部的唯一化符号。
    Unique(TermId),
}

/// 作用域帧：局部符号 → 绑定。
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

    /// 移除局部绑定。
    pub fn unbind(&mut self, symbol: SymbolId) {
        self.locals.remove(&symbol);
    }
}
