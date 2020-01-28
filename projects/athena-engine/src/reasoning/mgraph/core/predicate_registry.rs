//! Closed predicate registry (Living `26` / `29`).
//!
//! Descriptors document theory context and subject arity for each [`PredicateId`].
//! Admission and hyper-edge staging must consult this table — never invent string labels.

use std::ops::RangeInclusive;

use super::refs::{PredicateId, TheoryContextId, predicates};

/// Static descriptor for one closed [`PredicateId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredicateDescriptor {
    /// Predicate identity.
    pub id: PredicateId,
    /// Owning theory context.
    pub theory: TheoryContextId,
    /// Inclusive subject arity range (`SemanticRef` count on [`crate::reasoning::mgraph::RelationRecord`]).
    pub subject_arity: RangeInclusive<usize>,
}

const DESCRIPTORS: &[PredicateDescriptor] = &[
    PredicateDescriptor { id: predicates::POLYNOMIAL_RESULT, theory: TheoryContextId::POLYNOMIAL, subject_arity: 1..=1 },
    PredicateDescriptor { id: predicates::CONGRUENCE, theory: TheoryContextId::CONGRUENCE, subject_arity: 3..=3 },
    PredicateDescriptor { id: predicates::REWRITE_EQUIVALENT, theory: TheoryContextId::REWRITE, subject_arity: 2..=2 },
    PredicateDescriptor { id: predicates::EVALUATION_RESULT, theory: TheoryContextId::DEFAULT, subject_arity: 2..=2 },
    PredicateDescriptor { id: predicates::DERIVATIVE_OF, theory: TheoryContextId::CALCULUS, subject_arity: 3..=3 },
    PredicateDescriptor { id: predicates::SERIES_EXPANSION, theory: TheoryContextId::CALCULUS, subject_arity: 3..=3 },
    PredicateDescriptor { id: predicates::INTEGRAL_OF, theory: TheoryContextId::CALCULUS, subject_arity: 3..=3 },
];

/// Look up a closed predicate descriptor.
pub fn descriptor(id: PredicateId) -> Option<&'static PredicateDescriptor> {
    DESCRIPTORS.iter().find(|d| d.id == id)
}

/// Whether `subject_count` is legal for `id`.
pub fn arity_ok(id: PredicateId, subject_count: usize) -> bool {
    descriptor(id).is_some_and(|d| d.subject_arity.contains(&subject_count))
}

/// All registered descriptors (stable order by [`PredicateId`]).
pub fn all_descriptors() -> &'static [PredicateDescriptor] {
    DESCRIPTORS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_predicate_has_descriptor() {
        for id in [
            predicates::POLYNOMIAL_RESULT,
            predicates::CONGRUENCE,
            predicates::REWRITE_EQUIVALENT,
            predicates::EVALUATION_RESULT,
            predicates::DERIVATIVE_OF,
            predicates::SERIES_EXPANSION,
            predicates::INTEGRAL_OF,
        ] {
            assert!(descriptor(id).is_some(), "missing descriptor for {id:?}");
            assert!(arity_ok(id, *descriptor(id).unwrap().subject_arity.start()));
        }
    }

    #[test]
    fn unknown_predicate_has_no_descriptor() {
        assert!(descriptor(PredicateId(0)).is_none());
        assert!(descriptor(PredicateId(99)).is_none());
        assert!(!arity_ok(PredicateId(99), 2));
    }
}
