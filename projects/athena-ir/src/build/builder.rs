//! 受控 IR 构造 API。

use athena_numeric::NumericValue;
use athena_types::{CollectionKind, OperatorId, Result, SourceSpan, SymbolId, TermId};

use crate::{
    node::{Atom, MathematicalConstant, TermNode},
    operator::{ApplicationHead, OperatorRegistry, SemanticOperator},
    store::TermStore,
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

    /// 有序集合 term（显式 [`CollectionKind::OrderedCollection`]）。
    pub fn list(&mut self, items: Vec<TermId>, span: SourceSpan) -> TermId {
        self.collection(CollectionKind::OrderedCollection, items, span)
    }

    /// 带显式种类的集合 term。
    pub fn collection(&mut self, kind: CollectionKind, elements: Vec<TermId>, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Collection { kind, elements }, span)
    }

    /// 算子应用 term（semantic or extension head）。
    pub fn application(&mut self, head: ApplicationHead, args: Vec<TermId>, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Application { head, arguments: args }, span)
    }

    /// Core semantic operator application.
    pub fn application_semantic(&mut self, op: SemanticOperator, args: Vec<TermId>, span: SourceSpan) -> TermId {
        self.application(ApplicationHead::Semantic(op), args, span)
    }

    /// Extension operator application (display name via registry — never maps to core semantics).
    pub fn application_extension(
        &mut self,
        registry: &mut OperatorRegistry,
        head: &str,
        args: Vec<TermId>,
        span: SourceSpan,
    ) -> TermId {
        let op = registry.intern(head);
        self.application(ApplicationHead::Extension(op), args, span)
    }

    /// Extension application from an existing [`OperatorId`].
    pub fn application_extension_id(&mut self, op: OperatorId, args: Vec<TermId>, span: SourceSpan) -> TermId {
        self.application(ApplicationHead::Extension(op), args, span)
    }

    /// Typed Boolean 原子 term。
    pub fn boolean(&mut self, value: bool, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Atom(Atom::Boolean(value)), span)
    }

    /// Typed Null 原子 term。
    pub fn null(&mut self, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Atom(Atom::Null), span)
    }

    /// Closed mathematical constant atom.
    pub fn constant(&mut self, value: MathematicalConstant, span: SourceSpan) -> TermId {
        self.arena.push(TermNode::Atom(Atom::Constant(value)), span)
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
