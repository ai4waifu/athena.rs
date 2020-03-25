//! 类型化 term 构造 — 仅 [`SemanticOperator`] / [`UnaryFunction`] / 集合。

use athena_engine::{
    Session,
    runtime::values::arena::{push_constant, push_int, push_semantic, push_symbol_name},
};
use athena_ir::{MathematicalConstant, SemanticOperator, TermNode, UnaryFunction};
use athena_types::{CollectionKind, SymbolId, TermId};

/// 绑定到 `Session` 的 term 构造器，**无** 具名 head API。
pub struct TermBuilder<'a> {
    session: &'a mut Session,
}

impl<'a> TermBuilder<'a> {
    /// 绑定到 session。
    pub fn new(session: &'a mut Session) -> Self {
        Self { session }
    }

    /// 精确小整数原子。
    pub fn integer(&mut self, value: i64) -> TermId {
        push_int(self.session, value)
    }

    /// 按显示名构造符号原子（`SymbolId`，不是算子）。
    pub fn symbol(&mut self, name: &str) -> TermId {
        push_symbol_name(self.session, name)
    }

    /// 封闭数学常量（`Pi`、`E`、…）— 不是用户符号名。
    pub fn math_constant(&mut self, value: MathematicalConstant) -> TermId {
        push_constant(self.session, value)
    }

    /// intern 用户符号名（用于 [`DomainRequest`] / [`CalculusRequest`] 字段）。
    pub fn intern(&mut self, name: &str) -> SymbolId {
        self.session.arena.symbols_mut().intern(name)
    }

    /// 显式 [`CollectionKind::OrderedCollection`] 的有序集合。
    pub fn ordered(&mut self, elements: impl IntoIterator<Item = TermId>) -> TermId {
        let span = TermNode::default_span();
        self.session
            .arena
            .push(TermNode::Collection { kind: CollectionKind::OrderedCollection, elements: elements.into_iter().collect() }, span)
    }

    /// `SemanticOperator::Add`。
    pub fn add(&mut self, operands: impl IntoIterator<Item = TermId>) -> TermId {
        self.semantic(SemanticOperator::Add, operands)
    }

    /// `SemanticOperator::Subtract`。
    pub fn subtract(&mut self, left: TermId, right: TermId) -> TermId {
        self.semantic(SemanticOperator::Subtract, [left, right])
    }

    /// `SemanticOperator::Multiply`。
    pub fn multiply(&mut self, operands: impl IntoIterator<Item = TermId>) -> TermId {
        self.semantic(SemanticOperator::Multiply, operands)
    }

    /// `SemanticOperator::Divide`。
    pub fn divide(&mut self, numerator: TermId, denominator: TermId) -> TermId {
        self.semantic(SemanticOperator::Divide, [numerator, denominator])
    }

    /// `SemanticOperator::Power`。
    pub fn power(&mut self, base: TermId, exponent: TermId) -> TermId {
        self.semantic(SemanticOperator::Power, [base, exponent])
    }

    /// `SemanticOperator::Negate`。
    pub fn negate(&mut self, value: TermId) -> TermId {
        self.semantic(SemanticOperator::Negate, [value])
    }

    /// `SemanticOperator::Equal`。
    pub fn equal(&mut self, left: TermId, right: TermId) -> TermId {
        self.semantic(SemanticOperator::Equal, [left, right])
    }

    /// 经 [`SemanticOperator::from_unary`] 的封闭一元特殊函数。
    pub fn unary_function(&mut self, function: UnaryFunction, argument: TermId) -> TermId {
        self.semantic(SemanticOperator::from_unary(function), [argument])
    }

    /// 任意封闭语义应用。
    pub fn semantic(&mut self, operator: SemanticOperator, args: impl IntoIterator<Item = TermId>) -> TermId {
        push_semantic(self.session, operator, args.into_iter().collect())
    }
}
