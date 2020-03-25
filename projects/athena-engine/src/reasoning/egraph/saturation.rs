//! 带预算的饱和驱动器。

use athena_ir::TermStore;
use athena_rewriter::{PatternBindings, RuleSet, match_pattern, substitute};
use athena_types::TermId;

use super::{
    budget::{SaturationBudget, SaturationStopReason},
    candidate::CandidateEquivalence,
    graph::EGraph,
    typed_rules::TypedRuleSet,
};

/// 一次饱和尝试的报告。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct SaturationReport {
    /// 运行停止原因。
    pub stop: SaturationStopReason,
    /// 已执行的迭代次数。
    pub iterations: u32,
    /// 发现的候选等式（未验证）。
    pub candidates: Vec<CandidateEquivalence>,
}

impl SaturationReport {
    /// Owning 复制（[`CandidateEquivalence`] 为 `Copy`）。
    pub fn owning_copy(&self) -> Self {
        Self { stop: self.stop, iterations: self.iterations, candidates: self.candidates.clone() }
    }
}

/// 在 `budget` 下用结构化 [`RuleSet`] 规则运行作用域局部饱和。
///
/// 摄入 `roots` 后，扫描已知项，对每条规则模式做 [`TermStore::structural_eq`]
/// 命中检测，加入替换结果，发出
/// [`CandidateEquivalence`]，并在局部合并类。绝不写入 M-Graph。
pub fn saturate(
    graph: &mut EGraph,
    store: &TermStore,
    roots: &[TermId],
    budget: SaturationBudget,
    rules: Option<&RuleSet>,
) -> SaturationReport {
    if budget.max_iterations == 0 || budget.max_eclasses == 0 || budget.max_enodes == 0 {
        return SaturationReport { stop: SaturationStopReason::ResourceBudget, iterations: 0, candidates: Vec::new() };
    }

    let mut iterations = 0u32;
    let mut candidates = Vec::new();

    for root in roots {
        if over_structure_budget(graph, &budget) {
            return SaturationReport { stop: SaturationStopReason::ResourceBudget, iterations, candidates };
        }
        let _ = graph.add_term(store, *root);
        iterations = iterations.saturating_add(1);
        if iterations >= budget.max_iterations {
            return SaturationReport { stop: SaturationStopReason::IterationBudget, iterations, candidates };
        }
    }

    let Some(rules) = rules
    else {
        return SaturationReport { stop: SaturationStopReason::FixedPoint, iterations, candidates };
    };

    if rules.is_empty() {
        return SaturationReport { stop: SaturationStopReason::FixedPoint, iterations, candidates };
    }

    loop {
        if iterations >= budget.max_iterations {
            return SaturationReport { stop: SaturationStopReason::IterationBudget, iterations, candidates };
        }
        if over_structure_budget(graph, &budget) {
            return SaturationReport { stop: SaturationStopReason::ResourceBudget, iterations, candidates };
        }

        let mut progressed = false;
        let subjects = graph.known_terms();
        for rule in rules.iter() {
            for &subject in &subjects {
                if candidates.len() as u32 >= budget.max_candidate_unions {
                    return SaturationReport { stop: SaturationStopReason::ResourceBudget, iterations, candidates };
                }
                if !store.structural_eq(subject, rule.pattern) {
                    continue;
                }
                let Some(left_class) = graph.class_of_term(subject)
                else {
                    continue;
                };
                let Some(right_class) = graph.add_term(store, rule.replacement)
                else {
                    continue;
                };
                if graph.find(left_class) == graph.find(right_class) {
                    continue;
                }
                if already_emitted(&candidates, subject, rule.replacement) {
                    continue;
                }
                graph.union_classes(left_class, right_class);
                candidates.push(CandidateEquivalence {
                    left_term: subject,
                    right_term: rule.replacement,
                    left_class,
                    right_class,
                    rule: Some(rule.id),
                });
                progressed = true;
            }
        }

        iterations = iterations.saturating_add(1);
        if !progressed {
            return SaturationReport { stop: SaturationStopReason::FixedPoint, iterations, candidates };
        }
    }
}

/// 用 [`TypedRuleSet`]（`TermPattern` + `substitute`）运行作用域局部饱和。
///
/// 绝不写入 M-Graph。`store` 可变，以便经
/// [`athena_rewriter::substitute`] 实例化替换模板。
pub fn saturate_typed(
    graph: &mut EGraph,
    store: &mut TermStore,
    roots: &[TermId],
    budget: SaturationBudget,
    rules: Option<&TypedRuleSet>,
) -> SaturationReport {
    if budget.max_iterations == 0 || budget.max_eclasses == 0 || budget.max_enodes == 0 {
        return SaturationReport { stop: SaturationStopReason::ResourceBudget, iterations: 0, candidates: Vec::new() };
    }

    let mut iterations = 0u32;
    let mut candidates = Vec::new();

    for root in roots {
        if over_structure_budget(graph, &budget) {
            return SaturationReport { stop: SaturationStopReason::ResourceBudget, iterations, candidates };
        }
        let _ = graph.add_term(store, *root);
        iterations = iterations.saturating_add(1);
        if iterations >= budget.max_iterations {
            return SaturationReport { stop: SaturationStopReason::IterationBudget, iterations, candidates };
        }
    }

    let Some(rules) = rules
    else {
        return SaturationReport { stop: SaturationStopReason::FixedPoint, iterations, candidates };
    };

    if rules.is_empty() {
        return SaturationReport { stop: SaturationStopReason::FixedPoint, iterations, candidates };
    }

    loop {
        if iterations >= budget.max_iterations {
            return SaturationReport { stop: SaturationStopReason::IterationBudget, iterations, candidates };
        }
        if over_structure_budget(graph, &budget) {
            return SaturationReport { stop: SaturationStopReason::ResourceBudget, iterations, candidates };
        }

        let mut progressed = false;
        let subjects = graph.known_terms();
        for rule in rules.iter() {
            for &subject in &subjects {
                if candidates.len() as u32 >= budget.max_candidate_unions {
                    return SaturationReport { stop: SaturationStopReason::ResourceBudget, iterations, candidates };
                }
                let mut binds = PatternBindings::new();
                if !match_pattern(store, subject, &rule.pattern, &mut binds) {
                    continue;
                }
                let produced = substitute(store, rule.replacement, &binds);
                let Some(left_class) = graph.class_of_term(subject)
                else {
                    continue;
                };
                let Some(right_class) = graph.add_term(store, produced)
                else {
                    continue;
                };
                if graph.find(left_class) == graph.find(right_class) {
                    continue;
                }
                if already_emitted(&candidates, subject, produced) {
                    continue;
                }
                graph.union_classes(left_class, right_class);
                candidates.push(CandidateEquivalence {
                    left_term: subject,
                    right_term: produced,
                    left_class,
                    right_class,
                    rule: Some(rule.id),
                });
                progressed = true;
            }
        }

        iterations = iterations.saturating_add(1);
        if !progressed {
            return SaturationReport { stop: SaturationStopReason::FixedPoint, iterations, candidates };
        }
    }
}

fn over_structure_budget(graph: &EGraph, budget: &SaturationBudget) -> bool {
    graph.eclass_count() as u32 >= budget.max_eclasses || graph.enode_count() as u32 >= budget.max_enodes
}

fn already_emitted(candidates: &[CandidateEquivalence], left: TermId, right: TermId) -> bool {
    candidates.iter().any(|c| (c.left_term == left && c.right_term == right) || (c.left_term == right && c.right_term == left))
}
