//! Extract a representative term from an e-class (Living `13` / `26` bootstrap).

use athena_ir::{TermNode, TermStore};
use athena_types::TermId;

use crate::reasoning::mgraph::ExactUnionFind;

use super::{graph::EGraph, ids::EClassId};

/// Local extraction cost (not a solver multi-objective frontier).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultCost {
    /// DAG node count in [`TermStore`] (shared subtrees counted once).
    pub ast_nodes: u32,
    /// Whether this term is an ExactUF representative present in the e-class.
    pub admitted_exact: bool,
}

impl ResultCost {
    /// Lexicographic key: prefer admitted, then fewer AST nodes.
    pub fn rank_key(self) -> (u8, u32) {
        let admitted_rank = if self.admitted_exact { 0 } else { 1 };
        (admitted_rank, self.ast_nodes)
    }
}

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
    /// Lexicographic on [`ResultCost`]: admitted ExactUF reps first, then smallest AST.
    ResultCost,
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

    /// Score a term under optional ExactUF admission.
    pub fn score(store: &TermStore, term: TermId, exact_uf: Option<&ExactUnionFind>) -> ResultCost {
        let ast_nodes = ast_size(store, term);
        let admitted_exact = exact_uf.is_some_and(|uf| uf.find(term) == term);
        ResultCost {
            ast_nodes,
            admitted_exact,
        }
    }

    /// Extract a term for `class`.
    ///
    /// `exact_uf` is consulted for [`ExtractionPreference::AdmittedExact`] and
    /// [`ExtractionPreference::ResultCost`].
    pub fn extract(
        &self,
        graph: &EGraph,
        store: &TermStore,
        class: EClassId,
        exact_uf: Option<&ExactUnionFind>,
    ) -> Option<TermId> {
        self.extract_with_cost(graph, store, class, exact_uf)
            .map(|(term, _)| term)
    }

    /// Extract a term plus its [`ResultCost`].
    pub fn extract_with_cost(
        &self,
        graph: &EGraph,
        store: &TermStore,
        class: EClassId,
        exact_uf: Option<&ExactUnionFind>,
    ) -> Option<(TermId, ResultCost)> {
        let root = graph.find(class);
        let mut terms = graph.terms_in_class(root);
        if terms.is_empty() {
            let term = graph.term_for_class(root)?;
            return Some((term, Self::score(store, term, exact_uf)));
        }
        let chosen = match self.preference {
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
                        return reps
                            .into_iter()
                            .min_by_key(|t| (ast_size(store, *t), t.0))
                            .map(|t| (t, Self::score(store, t, exact_uf)));
                    }
                }
                terms.into_iter().min_by_key(|t| (ast_size(store, *t), t.0))
            }
            ExtractionPreference::ResultCost => {
                // Prefer ExactUF reps that still inhabit the class, then AST size.
                let class_terms = terms.clone();
                terms.into_iter().min_by_key(|t| {
                    let mut cost = Self::score(store, *t, exact_uf);
                    if let Some(uf) = exact_uf {
                        let rep = uf.find(*t);
                        cost.admitted_exact = rep == *t && class_terms.contains(&rep);
                    }
                    (cost.rank_key(), t.0)
                })
            }
        }?;
        Some((chosen, Self::score(store, chosen, exact_uf)))
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
