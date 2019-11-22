//! E-Graph bootstrap contracts (Living `26` / `29`).

use athena_ir::{SemanticOperator, TermNode};
use athena_types::SourceSpan;

use super::{
    EGraph, ExtractionPreference, Extractor, SaturationBudget, SaturationStopReason, saturate,
};

#[test]
fn add_term_builds_eclasses_without_mgraph_side_effects() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let one = store.push(TermNode::Atom(athena_ir::Atom::Number(
        athena_numeric::Number::small_int(1),
    )), span);
    let two = store.push(TermNode::Atom(athena_ir::Atom::Number(
        athena_numeric::Number::small_int(2),
    )), span);
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
    let report = saturate(&mut graph, &store, &[], SaturationBudget {
        max_iterations: 0,
        ..SaturationBudget::smoke()
    }, None);
    assert_eq!(report.stop, SaturationStopReason::ResourceBudget);
    assert!(report.candidates.is_empty());
}

#[test]
fn candidate_union_merges_classes_locally() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let a = store.push(TermNode::Atom(athena_ir::Atom::Number(
        athena_numeric::Number::small_int(1),
    )), span);
    let b = store.push(TermNode::Atom(athena_ir::Atom::Number(
        athena_numeric::Number::small_int(2),
    )), span);
    let mut graph = EGraph::new();
    let ca = graph.add_term(&store, a).unwrap();
    let cb = graph.add_term(&store, b).unwrap();
    assert_ne!(graph.find(ca), graph.find(cb));
    assert!(graph.union_classes(ca, cb));
    assert_eq!(graph.find(ca), graph.find(cb));
    let extracted = Extractor::with_preference(ExtractionPreference::FirstTerm)
        .extract(&graph, ca)
        .expect("term");
    assert!(extracted == a || extracted == b);
}

#[test]
fn saturate_adds_roots_to_fixed_point() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let x = store.push(TermNode::Atom(athena_ir::Atom::Number(
        athena_numeric::Number::small_int(0),
    )), span);
    let mut graph = EGraph::new();
    let report = saturate(&mut graph, &store, &[x], SaturationBudget::smoke(), None);
    assert_eq!(report.stop, SaturationStopReason::FixedPoint);
    assert!(graph.class_of_term(x).is_some());
    assert!(report.candidates.is_empty());
}
