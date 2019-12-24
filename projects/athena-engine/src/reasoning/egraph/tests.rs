//! E-Graph bootstrap contracts (Living `26` / `29`).

use athena_ir::{SemanticOperator, TermNode};
use athena_types::SourceSpan;

use super::{
    CandidateEquivalence, EGraph, ExtractionPreference, Extractor, SaturationBudget, SaturationStopReason,
    admit_structural_term_equality, candidate_to_outer, saturate, verify_structural_term_equality,
};
use crate::reasoning::mgraph::{Guarantee, ProofStepKind, Proposition, SemanticCore, VerificationPolicy};

#[test]
fn add_term_builds_eclasses_without_mgraph_side_effects() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let one = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(1))),
        span,
    );
    let two = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(2))),
        span,
    );
    let add = store.push(
        TermNode::Application {
            head: athena_ir::ApplicationHead::Semantic(SemanticOperator::Add),
            arguments: vec![one, two],
        },
        span,
    );

    let mut graph = EGraph::new();
    let class = graph.add_term(&store, add).expect("add");
    assert!(graph.eclass_count() >= 1);
    assert_eq!(graph.class_of_term(add), Some(class));
    assert_eq!(graph.term_for_class(class), Some(add));
}

#[test]
fn saturate_respects_zero_iteration_budget() {
    let store = athena_ir::TermStore::new();
    let mut graph = EGraph::new();
    let report = saturate(
        &mut graph,
        &store,
        &[],
        SaturationBudget {
            max_iterations: 0,
            ..SaturationBudget::smoke()
        },
        None,
    );
    assert_eq!(report.stop, SaturationStopReason::ResourceBudget);
    assert!(report.candidates.is_empty());
}

#[test]
fn candidate_union_merges_classes_locally() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let a = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(1))),
        span,
    );
    let b = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(2))),
        span,
    );
    let mut graph = EGraph::new();
    let ca = graph.add_term(&store, a).unwrap();
    let cb = graph.add_term(&store, b).unwrap();
    assert_ne!(graph.find(ca), graph.find(cb));
    assert!(graph.union_classes(ca, cb));
    assert_eq!(graph.find(ca), graph.find(cb));
    let extracted = Extractor::with_preference(ExtractionPreference::FirstTerm)
        .extract(&graph, &store, ca, None)
        .expect("term");
    assert!(extracted == a || extracted == b);
}

#[test]
fn saturate_adds_roots_to_fixed_point() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let x = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(0))),
        span,
    );
    let mut graph = EGraph::new();
    let report = saturate(&mut graph, &store, &[x], SaturationBudget::smoke(), None);
    assert_eq!(report.stop, SaturationStopReason::FixedPoint);
    assert!(graph.class_of_term(x).is_some());
    assert!(report.candidates.is_empty());
}

#[test]
fn candidate_to_outer_stays_unverified() {
    let left = athena_types::TermId(1);
    let right = athena_types::TermId(2);
    let outer = candidate_to_outer(&CandidateEquivalence {
        left_term: left,
        right_term: right,
        left_class: super::EClassId(0),
        right_class: super::EClassId(1),
        rule: None,
    });
    assert_eq!(outer.claim.guarantee, Guarantee::Candidate);
    assert!(matches!(
        outer.claim.proposition,
        Proposition::TermEquality { left: l, right: r } if l == left && r == right
    ));
}

#[test]
fn structural_admit_writes_exact_uf_and_proof_forest() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let a = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(7))),
        span,
    );
    let b = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(7))),
        span,
    );
    assert!(store.structural_eq(a, b));

    let claim = verify_structural_term_equality(&store, a, b).expect("verify");
    assert_eq!(claim.guarantee, Guarantee::ProvenExact);

    let mut semantic = SemanticCore::new();
    let fact = admit_structural_term_equality(&store, &mut semantic, a, b, &VerificationPolicy::default())
        .expect("admit");
    assert_eq!(fact.0, 0);
    assert_eq!(semantic.derived.exact_uf.find(a), semantic.derived.exact_uf.find(b));
    assert_eq!(semantic.derived.proof_forest.len(), 1);
    assert_eq!(
        semantic.derived.proof_forest.edges()[0].step_kind,
        ProofStepKind::AdmittedEquality
    );
}

#[test]
fn structural_verify_rejects_unequal_terms() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let a = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(1))),
        span,
    );
    let b = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(2))),
        span,
    );
    assert!(verify_structural_term_equality(&store, a, b).is_err());
}

#[test]
fn session_saturation_and_structural_admit() {
    use crate::runtime::Session;

    let mut session = Session::new();
    let span = SourceSpan::default();
    let a = session.arena.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(3))),
        span,
    );
    let b = session.arena.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(3))),
        span,
    );
    let report = session.run_egraph_saturation(&[a, b], None);
    assert_eq!(report.stop, SaturationStopReason::FixedPoint);
    assert!(session.egraph.class_of_term(a).is_some());
    session
        .admit_structural_term_equality(a, b)
        .expect("session admit");
    assert_eq!(
        session.mgraph.semantic.derived.exact_uf.find(a),
        session.mgraph.semantic.derived.exact_uf.find(b)
    );
}

#[test]
fn saturate_emits_candidates_from_structural_rule_match() {
    use athena_rewriter::RuleSet;

    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let zero = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(0))),
        span,
    );
    let one = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(1))),
        span,
    );
    let pattern = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(0))),
        span,
    );
    let replacement = one;
    let mut rules = RuleSet::new();
    let rule_id = rules.push(pattern, replacement, Some("zero_to_one"));

    let mut graph = EGraph::new();
    let report = saturate(&mut graph, &store, &[zero], SaturationBudget::smoke(), Some(&rules));
    assert_eq!(report.stop, SaturationStopReason::FixedPoint);
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].left_term, zero);
    assert_eq!(report.candidates[0].right_term, one);
    assert_eq!(report.candidates[0].rule, Some(rule_id));
    let witness = report.candidates[0].local_witness().expect("rule witness");
    assert_eq!(witness.rule, rule_id);
    assert_eq!(witness.subject, zero);
    assert_eq!(witness.produced, one);
    assert_eq!(
        graph.find(graph.class_of_term(zero).unwrap()),
        graph.find(graph.class_of_term(one).unwrap())
    );
}

#[test]
fn admit_structural_candidates_skips_rewrite_shaped_pairs() {
    use crate::runtime::Session;
    use athena_rewriter::RuleSet;

    let mut session = Session::new();
    let span = SourceSpan::default();
    let zero = session.arena.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(0))),
        span,
    );
    let one = session.arena.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(1))),
        span,
    );
    let pattern = session.arena.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(0))),
        span,
    );
    let mut rules = RuleSet::new();
    rules.push(pattern, one, Some("zero_to_one"));
    let report = session.run_egraph_saturation(&[zero], Some(&rules));
    assert_eq!(report.candidates.len(), 1);
    let admitted = session.admit_structural_egraph_candidates(&report.candidates);
    assert!(admitted.is_empty());
    assert_eq!(session.mgraph.semantic.derived.proof_forest.len(), 0);

    let twin = session.arena.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(0))),
        span,
    );
    let structural = CandidateEquivalence {
        left_term: zero,
        right_term: twin,
        left_class: session.egraph.class_of_term(zero).unwrap_or(super::EClassId(0)),
        right_class: super::EClassId(0),
        rule: None,
    };
    let admitted = session.admit_structural_egraph_candidates(&[structural]);
    assert_eq!(admitted.len(), 1);
    assert!(admitted[0].is_ok());
    assert_eq!(session.mgraph.semantic.derived.proof_forest.len(), 1);
}

#[test]
fn saturate_typed_binds_and_substitutes_replacement() {
    use athena_ir::{ApplicationHead, SemanticOperator};
    use crate::reasoning::trs::TermPattern;

    use super::{TypedRuleSet, saturate_typed};

    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let one = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(1))),
        span,
    );
    let add = store.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Add),
            arguments: vec![one, one],
        },
        span,
    );
    let x = store.symbols_mut().intern("x");
    let x_term = store.push(TermNode::Atom(athena_ir::Atom::Symbol(x)), span);
    let pattern = TermPattern::Application {
        operator: ApplicationHead::Semantic(SemanticOperator::Add),
        arguments: vec![
            TermPattern::Bind {
                name: x,
                inner: Box::new(TermPattern::Any),
            },
            TermPattern::Bind {
                name: x,
                inner: Box::new(TermPattern::Any),
            },
        ],
    };
    let mut rules = TypedRuleSet::new();
    let rule_id = rules.push(pattern, x_term, Some("add_same"));
    let mut graph = EGraph::new();
    let report = saturate_typed(&mut graph, &mut store, &[add], SaturationBudget::smoke(), Some(&rules));
    assert_eq!(report.stop, SaturationStopReason::FixedPoint);
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].left_term, add);
    assert_eq!(report.candidates[0].right_term, one);
    assert_eq!(report.candidates[0].rule, Some(rule_id));
    assert_eq!(
        graph.find(graph.class_of_term(add).unwrap()),
        graph.find(graph.class_of_term(one).unwrap())
    );
}

#[test]
fn extract_smallest_ast_prefers_shorter_term() {
    use athena_ir::{ApplicationHead, SemanticOperator};

    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let one = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(1))),
        span,
    );
    let add = store.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Add),
            arguments: vec![one, one],
        },
        span,
    );
    let mut graph = EGraph::new();
    let c_add = graph.add_term(&store, add).unwrap();
    let c_one = graph.add_term(&store, one).unwrap();
    assert!(graph.union_classes(c_add, c_one));
    let extracted = Extractor::with_preference(ExtractionPreference::SmallestAst)
        .extract(&graph, &store, c_add, None)
        .expect("term");
    assert_eq!(extracted, one);
}

#[test]
fn extract_admitted_exact_prefers_union_find_rep() {
    use crate::reasoning::mgraph::ExactUnionFind;
    use athena_ir::{ApplicationHead, SemanticOperator};

    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let one = store.push(
        TermNode::Atom(athena_ir::Atom::Number(athena_numeric::Number::small_int(1))),
        span,
    );
    let add = store.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Add),
            arguments: vec![one, one],
        },
        span,
    );
    let mut graph = EGraph::new();
    let c_add = graph.add_term(&store, add).unwrap();
    let c_one = graph.add_term(&store, one).unwrap();
    graph.union_classes(c_add, c_one);
    let mut uf = ExactUnionFind::default();
    // Force the admitted representative to be `one` (lower id after hash-cons still one).
    uf.union(add, one);
    let extracted = Extractor::with_preference(ExtractionPreference::AdmittedExact)
        .extract(&graph, &store, c_add, Some(&uf))
        .expect("term");
    assert_eq!(extracted, uf.find(one));
}
