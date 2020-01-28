//! 分层定义表与作用域帧（Living `25` / `27` · `SymbolId` 绑定 · `DispatchTableId` 规则）。
//!
//! - [`DefinitionLayer`]：语句层立即绑定 / 残余绑定 / 规则分派（Session 全局为底层）。
//! - [`ScopeFrame`]：局部遮蔽（读取优先，不写回全局）。
//! - 规则表按 [`DispatchTableId`] 存储；[`ExtensionOperatorId`] 仅作 Extension apply 索引（Living `27`）。

use std::collections::HashMap;

use athena_types::{DispatchTableId, ExtensionOperatorId, SymbolId, TermId};

use crate::reasoning::trs::TermPattern;

/// 语句层定义（立即绑定 / 残余绑定 / 规则分派）。
#[derive(Debug, Default)]
pub struct DefinitionLayer {
    bindings: HashMap<SymbolId, TermId>,
    residual_bindings: HashMap<SymbolId, TermId>,
    /// 规则分派表（Living `27` 一等键）。
    dispatch_tables: HashMap<DispatchTableId, Vec<(TermPattern, TermId)>>,
    /// Extension head → 其分派表（apply 路径索引，非字符串）。
    operator_tables: HashMap<ExtensionOperatorId, DispatchTableId>,
    /// `RegisterRuleDispatch` 头符号对其 Extension operator 的拥有关系。
    extension_rule_owners: HashMap<SymbolId, ExtensionOperatorId>,
    next_dispatch_table: u32,
}

impl DefinitionLayer {
    /// 空层。
    pub fn new() -> Self {
        Self::default()
    }

    /// 写入立即求值绑定（替换同符号的残余绑定与其拥有的 extension 规则）。
    pub fn write_binding(&mut self, symbol: SymbolId, value: TermId) {
        self.bindings.insert(symbol, value);
        self.residual_bindings.remove(&symbol);
        self.clear_owned_extension(symbol);
    }

    /// 写入残余项绑定（读取 / 应用时再求值）。
    pub fn write_residual_binding(&mut self, symbol: SymbolId, value: TermId) {
        self.residual_bindings.insert(symbol, value);
        self.bindings.remove(&symbol);
        self.clear_owned_extension(symbol);
    }

    /// 追加 extension head 规则（分配或复用 [`DispatchTableId`]）。
    pub fn register_extension_rule(&mut self, op: ExtensionOperatorId, pattern: TermPattern, replacement: TermId) {
        let table = self.ensure_table_for_operator(op);
        self.append_rule(table, pattern, replacement);
    }

    /// 分配独立分派表（供 `SessionCommand::RegisterRuleDispatch`）。
    pub fn alloc_dispatch_table(&mut self) -> DispatchTableId {
        let id = DispatchTableId(self.next_dispatch_table);
        self.next_dispatch_table = self.next_dispatch_table.saturating_add(1);
        self.dispatch_tables.entry(id).or_default();
        id
    }

    /// 将 Extension operator 绑定到已有分派表（apply 索引）。
    pub fn bind_operator_table(&mut self, op: ExtensionOperatorId, table: DispatchTableId) {
        self.operator_tables.insert(op, table);
        self.dispatch_tables.entry(table).or_default();
    }

    /// 向分派表追加一条规则。
    pub fn append_rule(&mut self, table: DispatchTableId, pattern: TermPattern, replacement: TermId) {
        self.dispatch_tables.entry(table).or_default().push((pattern, replacement));
    }

    /// 按表句柄查规则。
    pub fn rules_for_table(&self, table: DispatchTableId) -> Option<&[(TermPattern, TermId)]> {
        self.dispatch_tables.get(&table).map(Vec::as_slice)
    }

    /// 以用户符号拥有 extension 规则表（清除该符号值绑定，记录拥有关系）。
    pub fn register_extension_rule_for_symbol(&mut self, symbol: SymbolId, op: ExtensionOperatorId, pattern: TermPattern, replacement: TermId) {
        self.bindings.remove(&symbol);
        self.residual_bindings.remove(&symbol);
        self.extension_rule_owners.insert(symbol, op);
        self.register_extension_rule(op, pattern, replacement);
    }

    /// 查立即绑定。
    pub fn binding(&self, symbol: SymbolId) -> Option<TermId> {
        self.bindings.get(&symbol).copied()
    }

    /// 查残余绑定。
    pub fn residual_binding(&self, symbol: SymbolId) -> Option<TermId> {
        self.residual_bindings.get(&symbol).copied()
    }

    /// Extension apply：经 `ExtensionOperatorId` → [`DispatchTableId`] 取规则。
    pub fn extension_dispatch_rules(&self, op: ExtensionOperatorId) -> Option<&[(TermPattern, TermId)]> {
        let table = self.operator_tables.get(&op)?;
        self.dispatch_tables.get(table).map(Vec::as_slice)
    }

    /// 查 Extension head 对应的分派表句柄。
    pub fn dispatch_table_for(&self, op: ExtensionOperatorId) -> Option<DispatchTableId> {
        self.operator_tables.get(&op).copied()
    }

    /// 清除该符号的绑定及其拥有的 extension 规则。
    pub fn clear_symbol(&mut self, symbol: SymbolId) {
        self.bindings.remove(&symbol);
        self.residual_bindings.remove(&symbol);
        self.clear_owned_extension(symbol);
    }

    /// 清除 Extension head 对应的分派表。
    pub fn clear_extension(&mut self, op: ExtensionOperatorId) {
        if let Some(table) = self.operator_tables.remove(&op) {
            self.dispatch_tables.remove(&table);
        }
        self.extension_rule_owners.retain(|_, owned| *owned != op);
    }

    /// 清空层内全部定义。
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.residual_bindings.clear();
        self.dispatch_tables.clear();
        self.operator_tables.clear();
        self.extension_rule_owners.clear();
        self.next_dispatch_table = 0;
    }

    /// 层内是否存在该符号的任何绑定或规则拥有关系。
    pub fn defines(&self, symbol: SymbolId) -> bool {
        self.bindings.contains_key(&symbol) || self.residual_bindings.contains_key(&symbol) || self.extension_rule_owners.contains_key(&symbol)
    }

    fn ensure_table_for_operator(&mut self, op: ExtensionOperatorId) -> DispatchTableId {
        if let Some(id) = self.operator_tables.get(&op).copied() {
            return id;
        }
        let id = self.alloc_dispatch_table();
        self.operator_tables.insert(op, id);
        id
    }

    fn clear_owned_extension(&mut self, symbol: SymbolId) {
        if let Some(op) = self.extension_rule_owners.remove(&symbol) {
            self.clear_extension(op);
        }
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
