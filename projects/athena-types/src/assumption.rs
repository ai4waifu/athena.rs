//! Assumption sets for calculus and domain-conditioned results.

use crate::ids::{AssumptionSetId, SymbolId, TermId};

/// Atomic assumption predicate (language-neutral).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    /// `lhs = rhs`.
    Equal(TermId, TermId),
    /// `lhs ≠ rhs`.
    NotEqual(TermId, TermId),
    /// `lhs < rhs`.
    Less(TermId, TermId),
    /// `lhs ≤ rhs`.
    LessEqual(TermId, TermId),
    /// `lhs > rhs`.
    Greater(TermId, TermId),
    /// `lhs ≥ rhs`.
    GreaterEqual(TermId, TermId),
    /// Value is an integer.
    Integer(TermId),
    /// Value is strictly positive.
    Positive(TermId),
    /// Value is non-negative.
    NonNegative(TermId),
    /// Value is real.
    Real(TermId),
    /// Value is complex.
    Complex(TermId),
    /// Value is non-zero.
    NonZero(TermId),
    /// Symbol is non-zero (bridge until TermId binding lands).
    SymbolNonZero(SymbolId),
    /// Symbol is real.
    SymbolReal(SymbolId),
}

/// Ordered set of predicates attached to a request or result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssumptionSet {
    /// Stable id when stored in a session registry.
    pub id: Option<AssumptionSetId>,
    /// Predicates.
    pub predicates: Vec<Predicate>,
}

impl AssumptionSet {
    /// Empty assumption set.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build from predicates.
    pub fn from_predicates(predicates: Vec<Predicate>) -> Self {
        Self { id: None, predicates }
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
    }
}

/// A condition that qualifies a calculus (or domain) result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    /// Predicate that must hold for `value` to be valid.
    pub predicate: Predicate,
    /// Whether this condition was discharged by the engine.
    pub resolved: bool,
}
