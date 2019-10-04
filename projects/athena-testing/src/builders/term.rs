//! Typed term construction — only [`SemanticOperator`] / [`UnaryFunction`] / collections.

use athena_engine::Session;
use athena_engine::runtime::values::arena::{push_int, push_semantic, push_symbol_name};
use athena_ir::{SemanticOperator, TermNode, UnaryFunction};
use athena_types::{CollectionKind, SymbolId, TermId};

/// Session-bound term builder with **no** named-head API.
pub struct TermBuilder<'a> {
    session: &'a mut Session,
}

impl<'a> TermBuilder<'a> {
    /// Bind to a session.
    pub fn new(session: &'a mut Session) -> Self {
        Self { session }
    }

    /// Exact small integer atom.
    pub fn integer(&mut self, value: i64) -> TermId {
        push_int(self.session, value)
    }

    /// Symbol atom by display name (`SymbolId`, not an operator).
    pub fn symbol(&mut self, name: &str) -> TermId {
        push_symbol_name(self.session, name)
    }

    /// Intern a user symbol name (for [`DomainRequest`] / [`CalculusRequest`] fields).
    pub fn intern(&mut self, name: &str) -> SymbolId {
        self.session.arena.symbols_mut().intern(name)
    }

    /// Ordered collection with explicit [`CollectionKind::OrderedCollection`].
    pub fn ordered(&mut self, elements: impl IntoIterator<Item = TermId>) -> TermId {
        let span = TermNode::default_span();
        self.session.arena.push(
            TermNode::Collection {
                kind: CollectionKind::OrderedCollection,
                elements: elements.into_iter().collect(),
            },
            span,
        )
    }

    /// `SemanticOperator::Add`.
    pub fn add(&mut self, operands: impl IntoIterator<Item = TermId>) -> TermId {
        self.semantic(SemanticOperator::Add, operands)
    }

    /// `SemanticOperator::Subtract`.
    pub fn subtract(&mut self, left: TermId, right: TermId) -> TermId {
        self.semantic(SemanticOperator::Subtract, [left, right])
    }

    /// `SemanticOperator::Multiply`.
    pub fn multiply(&mut self, operands: impl IntoIterator<Item = TermId>) -> TermId {
        self.semantic(SemanticOperator::Multiply, operands)
    }

    /// `SemanticOperator::Divide`.
    pub fn divide(&mut self, numerator: TermId, denominator: TermId) -> TermId {
        self.semantic(SemanticOperator::Divide, [numerator, denominator])
    }

    /// `SemanticOperator::Power`.
    pub fn power(&mut self, base: TermId, exponent: TermId) -> TermId {
        self.semantic(SemanticOperator::Power, [base, exponent])
    }

    /// `SemanticOperator::Negate`.
    pub fn negate(&mut self, value: TermId) -> TermId {
        self.semantic(SemanticOperator::Negate, [value])
    }

    /// `SemanticOperator::Equal`.
    pub fn equal(&mut self, left: TermId, right: TermId) -> TermId {
        self.semantic(SemanticOperator::Equal, [left, right])
    }

    /// Closed unary special via [`SemanticOperator::from_unary`].
    pub fn unary_function(&mut self, function: UnaryFunction, argument: TermId) -> TermId {
        self.semantic(SemanticOperator::from_unary(function), [argument])
    }

    /// Arbitrary closed semantic application.
    pub fn semantic(&mut self, operator: SemanticOperator, args: impl IntoIterator<Item = TermId>) -> TermId {
        push_semantic(self.session, operator, args.into_iter().collect())
    }
}
