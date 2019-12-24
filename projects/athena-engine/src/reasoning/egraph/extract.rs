//! Extract a representative term from an e-class (Living `13` / `26` bootstrap).

use athena_ir::{TermNode, TermStore};
use athena_types::TermId;

use crate::reasoning::mgraph::ExactUnionFind;

use super::{graph::EGraph, ids::EClassId};

/// Preference for extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtractionPreference {
    /// Prefer the first recorded term root for the class (stable by [`TermId`] order).
    #[default]
    FirstTerm,
    /// Prefer the term with the fewest DAG nodes in [`TermStore`].
    SmallestAst,
    /// Prefer the [`ExactUnionFind`] representative when it still sits in the e-class.
    ///
    /// Falls back to [`Self::SmallestAst`] when no admitted representative is present.
    AdmittedExact,
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

    /// Extract a term for `class`.
    ///
    /// `exact_uf` is consulted only for [`ExtractionPreference::AdmittedExact`].
    pub fn extract(
        &self,
        graph: &EGraph,
        store: &TermStore,
        class: EClassId,
        exact_uf: Option<&ExactUnionFind>,
    ) -> Option<TermId> {
        let root = graph.find(class);
        let mut terms = graph.terms_in_class(root);
        if terms.is_empty() {
            return graph.term_for_class(root);
        }
        match self.preference {
            ExtractionPreference::FirstTerm => terms.into_iter().next(),
            ExtractionPreference::SmallestAst => terms
                .into_iter()
                .min_by_key(|t| (ast_size(store, *t), t.0)),
            ExtractionPreference::AdmittedExact => {
                if let Some(uf) = exact_uf {
                    let mut reps: Vec<TermId> = terms
                        .iter()
                        .map(|t| uf.find(*t))
                        .filter(|rep| terms.contains(rep))
                        .collect();
                    reps.sort_by_key(|t| t.0);
                    reps.dedup();
                    if !reps.is_empty() {
                        return reps.into_iter().min_by_key(|t| (ast_size(store, *t), t.0));
                    }
                }
                terms.into_iter().min_by_key(|t| (ast_size(store, *t), t.0))
            }
        }
    }
}

fn ast_size(store: &TermStore, root: TermId) -> u32 {
    let mut seen = std::collections::HashSet::new();
    ast_size_walk(store, root, &mut seen)
}

fn ast_size_walk(store: &TermStore, id: TermId, seen: &mut std::collections::HashSet<u32>) -> u32 {
    if !seen.insert(id.0) {
        return 0;
    }
    match store.get(id) {
        None => 1,
        Some(TermNode::Atom(_)) => 1,
        Some(TermNode::Collection { elements, .. }) => {
            1 + elements.iter().map(|c| ast_size_walk(store, *c, seen)).sum::<u32>()
        }
        Some(TermNode::Application { arguments, .. }) => {
            1 + arguments.iter().map(|c| ast_size_walk(store, *c, seen)).sum::<u32>()
        }
    }
}
