//! 受控 IR 构造 API。

use athena_numeric::NumericValue;
use athena_types::{OperatorId, Result, SourceSpan, SymbolId, TermId};

use crate::{
    store::TermStore,
    node::{Atom, TermNode},
    operator::OperatorRegistry,
    symbol::SymbolTable,
};

/// [`TermStore`] 构造器。
#[derive(Debug)]
pub struct TermBuilder<'a> {
    arena: &'a mut TermStore,
}

impl<'a> TermBuilder<'a> {
    /// 绑定 arena。
    pub fn new(arena: &'a mut TermStore) -> Self {
        Self { arena }
    }

    /// 数字原子 term。
    pub fn number(&mut self, n: NumericValue, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Atom(Atom::Number(n)), span)
    }

    /// 字符串原子 term。
    pub fn string(&mut self, s: impl Into<String>, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Atom(Atom::String(s.into())), span)
    }

    /// 符号原子 term（intern 名称）。
    pub fn symbol(&mut self, name: impl Into<String>, span: SourceSpan) -> TermId {
        let id = self.arena.symbols_mut().intern(name);
        self.symbol_id(id, span)
    }

    /// 已有符号 id 的原子 term。
    pub fn symbol_id(&mut self, symbol: SymbolId, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Atom(Atom::Symbol(symbol)), span)
    }

    /// 列表 term。
    pub fn list(&mut self, items: Vec<TermId>, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::List(items), span)
    }

    /// 算子应用 term。
    pub fn application(&mut self, op: OperatorId, args: Vec<TermId>, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Application { head: op, arguments: args }, span)
    }

    /// 经注册表解析 head 名的应用 term。
    pub fn application_named(&mut self, registry: &mut OperatorRegistry, head: &str, args: Vec<TermId>, span: SourceSpan) -> TermId {
        let op = registry.intern(head);
        self.application(op, args, span)
    }

    /// Typed Boolean 原子 term。
    pub fn boolean(&mut self, value: bool, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Atom(Atom::Boolean(value)), span)
    }

    /// Typed Null 原子 term。
    pub fn null(&mut self, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Atom(Atom::Null), span)
    }

    /// 小型精确整数。
    pub fn int(&mut self, n: i64, span: SourceSpan) -> TermId {
        self.number(NumericValue::small_int(n), span)
    }

    /// 精确有理数原子 term（`i64` 分子分母）。
    pub fn rational_i64(&mut self, num: i64, den: i64, span: SourceSpan) -> Result<TermId> {
        Ok(self.number(NumericValue::rational_i64(num, den)?, span))
    }

    /// 机器实数原子 term（由已解码 `f64`）。
    pub fn real(&mut self, x: f64, span: SourceSpan) -> TermId {
        self.number(NumericValue::machine(x), span)
    }

    /// 符号表（只读）。
    pub fn symbols(&self) -> &SymbolTable {
        self.arena.symbols()
    }
}
