//! Extract a representative term from an e-class (bootstrap stub).

use athena_types::TermId;

use super::{graph::EGraph, ids::EClassId};

/// Preference for extraction (cost model filled later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtractionPreference {
    /// Prefer the first recorded term root for the class.
    #[default]
    FirstTerm,
}

/// Extracts a host [`TermId`] from a local e-class when available.
#[derive(Debug, Default)]
pub struct Extractor {
    preference: ExtractionPreference,
}

impl Extractor {
    /// Default extractor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct with preference.
    pub fn with_preference(preference: ExtractionPreference) -> Self {
        Self { preference }
    }

    /// Extract a term for `class` if the bootstrap graph recorded one.
    ///
    /// Full cost-based extraction (Living `13` ResultCost / Pareto) is not implemented.
    pub fn extract(&self, graph: &EGraph, class: EClassId) -> Option<TermId> {
        let _ = self.preference;
        let root = graph.find(class);
        // Scan term_class map via public API: class_of_term is reverse — use enode terms.
        graph.term_for_class(root)
    }
}
