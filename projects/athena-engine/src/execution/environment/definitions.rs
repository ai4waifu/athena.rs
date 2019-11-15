//! 分层定义表与作用域帧（Living `25` / `27` · `SymbolId` 绑定 · `OperatorId` 规则）。
//!
//! - [`DefinitionLayer`]：语句层立即绑定 / 残余绑定 / 规则分派（Session 全局为底层）。
//! - [`ScopeFrame`]：局部遮蔽（读取优先，不写回全局）。
//! - 规则分派只按 [`OperatorId`] 索引（Living `27`，禁止字符串桥接 / `SymbolId` 平行表）。

use std::collections::HashMap;

use athena_types::{OperatorId, SymbolId, TermId};

use crate::reasoning::trs::TermPattern;

/// 语句层定义（立即绑定 / 残余绑定 / 规则分派）。
#[derive(Debug, Default)]
pub struct DefinitionLayer {
    bindings: HashMap<SymbolId, TermId>,
    residual_bindings: HashMap<SymbolId, TermId>,
    /// Extension / 表面 head 规则（Living `27`）。
    extension_dispatch_rules: HashMap<OperatorId, Vec<(TermPattern, TermId)>>,
}

impl DefinitionLayer {
    /// 空层。
    pub fn new() -> Self {
        Self::default()
    }

    /// 写入立即求值绑定（替换同符号的残余绑定）。
    pub fn write_binding(&mut self, symbol: SymbolId, value: TermId) {
        self.bindings.insert(symbol, value);
        self.residual_bindings.remove(&symbol);
    }

    /// 写入残余项绑定（读取 / 应用时再求值）。
    pub fn write_residual_binding(&mut self, symbol: SymbolId, value: TermId) {
        self.residual_bindings.insert(symbol, value);
        self.bindings.remove(&symbol);
    }

    /// 追加 extension head 规则（[`OperatorId`] 键）。写入规则时清除同符号立即绑定需由调用方处理。
    pub fn register_extension_rule(&mut self, op: OperatorId, pattern: TermPattern, replacement: TermId) {
        self.extension_dispatch_rules.entry(op).or_default().push((pattern, replacement));
    }

    /// 查立即绑定。
    pub fn binding(&self, symbol: SymbolId) -> Option<TermId> {
        self.bindings.get(&symbol).copied()
    }

    /// 查残余绑定。
    pub fn residual_binding(&self, symbol: SymbolId) -> Option<TermId> {
        self.residual_bindings.get(&symbol).copied()
    }

    /// 查 extension head 规则分派表。
    pub fn extension_dispatch_rules(&self, op: OperatorId) -> Option<&[(TermPattern, TermId)]> {
        self.extension_dispatch_rules.get(&op).map(Vec::as_slice)
    }

    /// 清除该符号的绑定（不含 extension 规则；规则按 [`OperatorId`] 清除）。
    pub fn clear_symbol(&mut self, symbol: SymbolId) {
        self.bindings.remove(&symbol);
        self.residual_bindings.remove(&symbol);
    }

    /// 清除 extension head 的分派规则。
    pub fn clear_extension(&mut self, op: OperatorId) {
        self.extension_dispatch_rules.remove(&op);
    }

    /// 清空层内全部定义。
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.residual_bindings.clear();
        self.extension_dispatch_rules.clear();
    }

    /// 层内是否存在该符号的任何绑定。
    pub fn defines(&self, symbol: SymbolId) -> bool {
        self.bindings.contains_key(&symbol) || self.residual_bindings.contains_key(&symbol)
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
