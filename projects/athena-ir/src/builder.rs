//! 受控 IR 构造 API。

use athena_types::{Number, OperatorId, SourceSpan, SymbolId, TermId};

use crate::{
    arena::TermArena,
    node::{AtomKind, TermKind},
    symbol::SymbolTable,
};

/// [`TermArena`] 构造器。
#[derive(Debug)]
pub struct TermBuilder<'a> {
    arena: &'a mut TermArena,
}

impl<'a> TermBuilder<'a> {
    /// 绑定 arena。
    pub fn new(arena: &'a mut TermArena) -> Self {
        Self { arena }
    }

    /// 数字原子 term。
    pub fn number(&mut self, n: Number, span: SourceSpan) -> TermId {
        self.arena.push(TermKind::Atom(AtomKind::Number(n)), span)
    }

    /// 字符串原子 term。
    pub fn string(&mut self, s: impl Into<String>, span: SourceSpan) -> TermId {
        self.arena.push(TermKind::Atom(AtomKind::String(s.into())), span)
    }

    /// 符号原子 term（intern 名称）。
    pub fn symbol(&mut self, name: impl Into<String>, span: SourceSpan) -> TermId {
        let id = self.arena.symbols_mut().intern(name);
        self.symbol_id(id, span)
    }

    /// 已有符号 id 的原子 term。
    pub fn symbol_id(&mut self, sym: SymbolId, span: SourceSpan) -> TermId {
        self.arena.push(TermKind::Atom(AtomKind::Symbol(sym)), span)
    }

    /// 列表 term。
    pub fn list(&mut self, items: Vec<TermId>, span: SourceSpan) -> TermId {
        self.arena.push(TermKind::List(items), span)
    }

    /// 算子应用 term。
    pub fn app(&mut self, op: OperatorId, args: Vec<TermId>, span: SourceSpan) -> TermId {
        self.arena.push(TermKind::App { op, args }, span)
    }

    /// 符号表（只读）。
    pub fn symbols(&self) -> &SymbolTable {
        self.arena.symbols()
    }
}
