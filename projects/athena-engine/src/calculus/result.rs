//! Calculus result contracts — expression + conditions + completeness.

use athena_types::{AssumptionSet, Condition, Diagnostic, Predicate};

use crate::term::Term;

/// Result carrying a value plus applicability conditions.
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalResult<T> {
    /// Computed value.
    pub value: T,
    /// Conditions under which `value` is valid.
    pub conditions: Vec<Condition>,
    /// Conditions the engine could not discharge.
    pub unresolved: Vec<Condition>,
}

impl<T> ConditionalResult<T> {
    /// Exact result with no conditions.
    pub fn exact(value: T) -> Self {
        Self { value, conditions: Vec::new(), unresolved: Vec::new() }
    }

    /// Result with unresolved predicates (caller must not treat as unconditional).
    pub fn with_unresolved(value: T, unresolved: Vec<Condition>) -> Self {
        Self { value, conditions: Vec::new(), unresolved }
    }
}

/// Unified calculus outcome (not a bare [`Term`]).
#[derive(Debug, Clone, PartialEq)]
pub enum CalculusResult<T = Term> {
    /// Exact symbolic result.
    Exact {
        /// Value.
        value: T,
        /// Discharged conditions.
        conditions: Vec<Condition>,
    },
    /// Result valid only under listed conditions.
    Conditional {
        /// Value.
        value: T,
        /// Conditions.
        conditions: Vec<Condition>,
    },
    /// Left unevaluated with a structured reason.
    Unevaluated {
        /// Original or residual expression.
        expression: T,
        /// Why evaluation stopped.
        reason: Diagnostic,
    },
}

impl CalculusResult<Term> {
    /// Convert a [`ConditionalResult`] into the public enum.
    pub fn from_conditional(c: ConditionalResult<Term>) -> Self {
        if c.unresolved.is_empty() && c.conditions.is_empty() {
            Self::Exact { value: c.value, conditions: Vec::new() }
        } else if c.unresolved.is_empty() {
            Self::Conditional { value: c.value, conditions: c.conditions }
        } else {
            let mut conditions = c.conditions;
            conditions.extend(c.unresolved.iter().cloned());
            Self::Conditional { value: c.value, conditions }
        }
    }
}

/// Build unresolved conditions from an assumption set that was not fully used.
pub fn unresolved_from_assumptions(set: &AssumptionSet) -> Vec<Condition> {
    set.predicates
        .iter()
        .cloned()
        .map(|predicate| Condition { predicate, resolved: false })
        .collect()
}

/// Helper: mark a predicate as unresolved.
pub fn unresolved(predicate: Predicate) -> Condition {
    Condition { predicate, resolved: false }
}
