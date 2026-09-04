//! 受控 IR 构造 API。

use athena_numeric::NumericValue;
use athena_types::{ExprId, OperatorId, Result, SourceSpan, SymbolId};

use crate::{
    arena::ExprArena,
    node::{Atom, ExprNode},
    operator::OperatorRegistry,
    symbol::SymbolTable,
};

/// [`ExprArena`] 构造器。
#[derive(Debug)]
pub struct ExprBuilder<'a> {
    arena: &'a mut ExprArena,
}

impl<'a> ExprBuilder<'a> {
    /// 绑定 arena。
    pub fn new(arena: &'a mut ExprArena) -> Self {
        Self { arena }
    }

    /// 数字原子 term。
    pub fn number(&mut self, n: NumericValue, span: SourceSpan) -> ExprId {
        self.arena.push(ExprNode::Atom(Atom::Number(n)), span)
    }

    /// 字符串原子 term。
    pub fn string(&mut self, s: impl Into<String>, span: SourceSpan) -> ExprId {
        self.arena.push(ExprNode::Atom(Atom::String(s.into())), span)
    }

    /// 符号原子 term（intern 名称）。
    pub fn symbol(&mut self, name: impl Into<String>, span: SourceSpan) -> ExprId {
        let id = self.arena.symbols_mut().intern(name);
        self.symbol_id(id, span)
    }

    /// 已有符号 id 的原子 term。
    pub fn symbol_id(&mut self, sym: SymbolId, span: SourceSpan) -> ExprId {
        self.arena.push(ExprNode::Atom(Atom::Symbol(sym)), span)
    }

    /// 列表 term。
    pub fn list(&mut self, items: Vec<ExprId>, span: SourceSpan) -> ExprId {
        self.arena.push(ExprNode::List(items), span)
    }

    /// 算子应用 term。
    pub fn app(&mut self, op: OperatorId, args: Vec<ExprId>, span: SourceSpan) -> ExprId {
        self.arena.push(ExprNode::App { op, args }, span)
    }

    /// 经注册表解析 head 名的应用 term。
    pub fn app_named(&mut self, registry: &mut OperatorRegistry, head: &str, args: Vec<ExprId>, span: SourceSpan) -> ExprId {
        let op = registry.intern(head);
        self.app(op, args, span)
    }

    /// Typed Boolean 原子 term。
    pub fn boolean(&mut self, value: bool, span: SourceSpan) -> ExprId {
        self.arena.push(ExprNode::Atom(Atom::Boolean(value)), span)
    }

    /// Typed Null 原子 term。
    pub fn null(&mut self, span: SourceSpan) -> ExprId {
        self.arena.push(ExprNode::Atom(Atom::Null), span)
    }

    /// 小型精确整数。
    pub fn int(&mut self, n: i64, span: SourceSpan) -> ExprId {
        self.number(NumericValue::small_int(n), span)
    }

    /// 精确有理数原子 term（`i64` 分子分母）。
    pub fn rational_i64(&mut self, num: i64, den: i64, span: SourceSpan) -> Result<ExprId> {
        Ok(self.number(NumericValue::rational_i64(num, den)?, span))
    }

    /// 机器实数原子 term（由已解码 `f64`）。
    pub fn real(&mut self, x: f64, span: SourceSpan) -> ExprId {
        self.number(NumericValue::machine(x), span)
    }

    /// 符号表（只读）。
    pub fn symbols(&self) -> &SymbolTable {
        self.arena.symbols()
    }
}
