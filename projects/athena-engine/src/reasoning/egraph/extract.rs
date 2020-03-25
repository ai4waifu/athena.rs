//! 从 e-class 抽取代表项（引导实现）。

use athena_ir::{TermNode, TermStore};
use athena_types::TermId;

use crate::reasoning::mgraph::ExactUnionFind;

use super::{graph::EGraph, ids::EClassId};

/// 局部抽取代价（单胜者与 Pareto 抽取的目标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultCost {
    /// [`TermStore`] 中的 DAG 节点数（共享子树只计一次）。
    pub ast_nodes: u32,
    /// 该项是否为仍在该 e-class 中的 ExactUF 代表元。
    pub admitted_exact: bool,
}

impl ResultCost {
    /// 字典序键：优先已接纳，再优先更少的 AST 节点。
    pub fn rank_key(self) -> (u8, u32) {
        let admitted_rank = if self.admitted_exact { 0 } else { 1 };
        (admitted_rank, self.ast_nodes)
    }

    /// 在 `(admitted_exact 最大化, ast_nodes 最小化)` 上的 Pareto 支配。
    ///
    /// 当 `self` 在每个目标上都不劣于 `other`，且至少在一个目标上严格更优时，
    /// `self` 支配 `other`。
    pub fn dominates(self, other: Self) -> bool {
        let adm_ge = (self.admitted_exact as u8) >= (other.admitted_exact as u8);
        let ast_le = self.ast_nodes <= other.ast_nodes;
        let adm_gt = self.admitted_exact && !other.admitted_exact;
        let ast_lt = self.ast_nodes < other.ast_nodes;
        adm_ge && ast_le && (adm_gt || ast_lt)
    }
}

/// 单个 e-class 的非支配抽取候选集（Pareto 引导实现）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq, Default)]
pub struct ParetoFrontier {
    /// 未被支配的 `(term, cost)` 点，按 [`ResultCost::rank_key`] 再按 [`TermId`] 排序。
    pub points: Vec<(TermId, ResultCost)>,
}

impl ParetoFrontier {
    /// Owning 复制（`(TermId, ResultCost)` 均为 `Copy`）。
    pub fn owning_copy(&self) -> Self {
        Self { points: self.points.clone() }
    }

    /// 前沿是否为空。
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// 非支配点的数量。
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// 字典序单点选取（顺序同 [`ExtractionPreference::ResultCost`]）。
    pub fn lexicographic_pick(&self) -> Option<(TermId, ResultCost)> {
        self.points.first().copied()
    }
}

/// 抽取偏好。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtractionPreference {
    /// 优先该类首次记录的项根（按 [`TermId`] 顺序稳定）。
    #[default]
    FirstTerm,
    /// 优先 [`TermStore`] 中 DAG 节点数最少的项。
    SmallestAst,
    /// 当 [`ExactUnionFind`] 代表元仍在该 e-class 中时优先选取。
    ///
    /// 若无已接纳代表元，则回退到 [`Self::SmallestAst`]。
    AdmittedExact,
    /// 对 [`ResultCost`] 做字典序：先 ExactUF 代表元，再最小 AST。
    ResultCost,
}

/// 在可用时从局部 e-class 抽取宿主 [`TermId`]。
#[derive(Debug, Default)]
pub struct Extractor {
    preference: ExtractionPreference,
}

impl Extractor {
    /// 默认抽取器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 按偏好构造。
    pub fn with_preference(preference: ExtractionPreference) -> Self {
        Self { preference }
    }

    /// 在可选 ExactUF 接纳信息下为项打分。
    pub fn score(store: &TermStore, term: TermId, exact_uf: Option<&ExactUnionFind>) -> ResultCost {
        let ast_nodes = ast_size(store, term);
        let admitted_exact = exact_uf.is_some_and(|uf| uf.find(term) == term);
        ResultCost { ast_nodes, admitted_exact }
    }

    /// 结合 e-class 成员关系为 ExactUF 代表元打分。
    fn score_in_class(store: &TermStore, term: TermId, class_terms: &[TermId], exact_uf: Option<&ExactUnionFind>) -> ResultCost {
        let mut cost = Self::score(store, term, exact_uf);
        if let Some(uf) = exact_uf {
            let rep = uf.find(term);
            cost.admitted_exact = rep == term && class_terms.contains(&rep);
        }
        cost
    }

    /// 为 `class` 抽取一项。
    ///
    /// [`ExtractionPreference::AdmittedExact`] 与
    /// [`ExtractionPreference::ResultCost`] 会查阅 `exact_uf`。
    pub fn extract(&self, graph: &EGraph, store: &TermStore, class: EClassId, exact_uf: Option<&ExactUnionFind>) -> Option<TermId> {
        self.extract_with_cost(graph, store, class, exact_uf).map(|(term, _)| term)
    }

    /// 抽取一项及其 [`ResultCost`]。
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

    /// 为 `class` 做多目标非支配抽取集合（不选出单一胜者）。
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
