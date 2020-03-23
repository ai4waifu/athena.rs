//! Extract a representative term from an e-class (Living `13` / `26` bootstrap).

use athena_ir::{TermNode, TermStore};
use athena_types::TermId;

use crate::reasoning::mgraph::ExactUnionFind;

use super::{graph::EGraph, ids::EClassId};

/// Local extraction cost (objectives for single-winner and Pareto extract).
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

    /// Pareto dominance on `(admitted_exact maximize, ast_nodes minimize)`.
    ///
    /// `self` dominates `other` when it is at least as good on every objective and
    /// strictly better on at least one.
    pub fn dominates(self, other: Self) -> bool {
        let adm_ge = (self.admitted_exact as u8) >= (other.admitted_exact as u8);
        let ast_le = self.ast_nodes <= other.ast_nodes;
        let adm_gt = self.admitted_exact && !other.admitted_exact;
        let ast_lt = self.ast_nodes < other.ast_nodes;
        adm_ge && ast_le && (adm_gt || ast_lt)
    }
}

/// Non-dominated extract candidates for one e-class (Living `16` Pareto bootstrap).
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq, Default)]
pub struct ParetoFrontier {
    /// Undominated `(term, cost)` points, sorted by [`ResultCost::rank_key`] then [`TermId`].
    pub points: Vec<(TermId, ResultCost)>,
}

impl ParetoFrontier {
    /// Owning 复制（Living `31`：`(TermId, ResultCost)` 均为 `Copy`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            points: self.points.clone(),
        }
    }

    /// Whether the frontier is empty.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Number of undominated points.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Lexicographic single pick (same order as [`ExtractionPreference::ResultCost`]).
    pub fn lexicographic_pick(&self) -> Option<(TermId, ResultCost)> {
        self.points.first().copied()
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
        ResultCost { ast_nodes, admitted_exact }
    }

    /// Score with e-class membership for ExactUF representatives.
    fn score_in_class(store: &TermStore, term: TermId, class_terms: &[TermId], exact_uf: Option<&ExactUnionFind>) -> ResultCost {
        let mut cost = Self::score(store, term, exact_uf);
        if let Some(uf) = exact_uf {
            let rep = uf.find(term);
            cost.admitted_exact = rep == term && class_terms.contains(&rep);
        }
        cost
    }

    /// Extract a term for `class`.
    ///
    /// `exact_uf` is consulted for [`ExtractionPreference::AdmittedExact`] and
    /// [`ExtractionPreference::ResultCost`].
    pub fn extract(&self, graph: &EGraph, store: &TermStore, class: EClassId, exact_uf: Option<&ExactUnionFind>) -> Option<TermId> {
        self.extract_with_cost(graph, store, class, exact_uf).map(|(term, _)| term)
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
            ExtractionPreference::SmallestAst => terms.into_iter().min_by_key(|t| (ast_size(store, *t), t.0)),
            ExtractionPreference::AdmittedExact => {
                if let Some(uf) = exact_uf {
                    let mut reps: Vec<TermId> = terms.iter().map(|t| uf.find(*t)).filter(|rep| terms.contains(rep)).collect();
                    reps.sort_by_key(|t| t.0);
                    reps.dedup();
                    if !reps.is_empty() {
                        return reps.into_iter().min_by_key(|t| (ast_size(store, *t), t.0)).map(|t| (t, Self::score(store, t, exact_uf)));
                    }
                }
                terms.into_iter().min_by_key(|t| (ast_size(store, *t), t.0))
            }
            ExtractionPreference::ResultCost => {
                let class_terms = terms.clone();
                terms.into_iter().min_by_key(|t| {
                    let cost = Self::score_in_class(store, *t, &class_terms, exact_uf);
                    (cost.rank_key(), t.0)
                })
            }
        }?;
        Some((chosen, Self::score(store, chosen, exact_uf)))
    }

    /// Multi-objective undominated extract set for `class` (does not pick a single winner).
    pub fn extract_pareto(graph: &EGraph, store: &TermStore, class: EClassId, exact_uf: Option<&ExactUnionFind>) -> ParetoFrontier {
        let root = graph.find(class);
        let mut terms = graph.terms_in_class(root);
        if terms.is_empty() {
            if let Some(term) = graph.term_for_class(root) {
                terms.push(term);
            }
        }
        let scored: Vec<(TermId, ResultCost)> = terms.iter().copied().map(|t| (t, Self::score_in_class(store, t, &terms, exact_uf))).collect();
        let mut points: Vec<(TermId, ResultCost)> = scored
            .iter()
            .copied()
            .filter(|(term, cost)| !scored.iter().any(|(other_term, other_cost)| other_term != term && other_cost.dominates(*cost)))
            .collect();
        points.sort_by_key(|(term, cost)| (cost.rank_key(), term.0));
        points.dedup_by_key(|(term, _)| *term);
        ParetoFrontier { points }
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
        Some(TermNode::Collection { elements, .. }) => 1 + elements.iter().map(|c| ast_size_walk(store, *c, seen)).sum::<u32>(),
        Some(TermNode::Application { arguments, .. }) => 1 + arguments.iter().map(|c| ast_size_walk(store, *c, seen)).sum::<u32>(),
    }
}
