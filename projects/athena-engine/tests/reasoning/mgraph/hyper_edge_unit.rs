//! 自 `src/reasoning/mgraph/admission/hyper_edge.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::polynomial::PolynomialCacheOp,
    reasoning::mgraph::{CalculusRelationKind, Evidence, Guarantee, HyperEdge, PredicateId, Proposition, admission::*, predicates},
};
use athena_ir::{Atom, TermNode, TermStore, canonical_hash};
use athena_types::{SourceSpan, TermId};

fn store_with_symbols() -> (TermStore, TermId, TermId, TermId) {
    let mut store = TermStore::new();
    let span = SourceSpan::default();
    let a = store.symbols_mut().intern("a");
    let b = store.symbols_mut().intern("b");
    let c = store.symbols_mut().intern("c");
    let t0 = store.push(TermNode::Atom(Atom::Symbol(a)), span);
    let t1 = store.push(TermNode::Atom(Atom::Symbol(b)), span);
    let t2 = store.push(TermNode::Atom(Atom::Symbol(c)), span);
    (store, t0, t1, t2)
}

#[test]
fn rewrite_hyper_edge_stages_candidate_term_equality() {
    let (store, left, right, _) = store_with_symbols();
    let edge = HyperEdge { nodes: vec![left, right], predicate: predicates::REWRITE_EQUIVALENT };
    let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
    assert_eq!(outer.claim.guarantee, Guarantee::Candidate);
    assert_eq!(outer.claim.proposition, Proposition::TermEquality { left, right });
}

#[test]
fn evaluation_result_hyper_edge_stages_term_equality() {
    let (store, left, right, _) = store_with_symbols();
    let edge = HyperEdge { nodes: vec![left, right], predicate: predicates::EVALUATION_RESULT };
    let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
    match &outer.claim.evidence {
        Evidence::TrustedKernel { summary, .. } => {
            assert!(summary.starts_with("hyper-edge-eval:"));
        }
    }
}

#[test]
fn derivative_hyper_edge_stages_calculus_relation() {
    let (store, expr, var, result) = store_with_symbols();
    let edge = HyperEdge { nodes: vec![expr, var, result], predicate: predicates::DERIVATIVE_OF };
    let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
    match outer.claim.proposition {
        Proposition::CalculusRelation { kind, expression_fingerprint, variable_fingerprint, result_term } => {
            assert_eq!(kind, CalculusRelationKind::DerivativeOf);
            assert_eq!(expression_fingerprint, canonical_hash(&store, expr));
            assert_eq!(variable_fingerprint, canonical_hash(&store, var));
            assert_eq!(result_term, result);
        }
        other => panic!("expected CalculusRelation, got {other:?}"),
    }
}

#[test]
fn congruence_hyper_edge_stages_fingerprints() {
    let (store, left, right, modulus) = store_with_symbols();
    let edge = HyperEdge { nodes: vec![left, right, modulus], predicate: predicates::CONGRUENCE };
    let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
    assert_eq!(
        outer.claim.proposition,
        Proposition::Congruence {
            left: canonical_hash(&store, left),
            right: canonical_hash(&store, right),
            modulus_fingerprint: canonical_hash(&store, modulus),
        }
    );
}

#[test]
fn polynomial_result_hyper_edge_stages_request_fingerprint() {
    let (store, request, _, _) = store_with_symbols();
    let edge = HyperEdge { nodes: vec![request], predicate: predicates::POLYNOMIAL_RESULT };
    let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
    assert_eq!(
        outer.claim.proposition,
        Proposition::PolynomialResult { operation: PolynomialCacheOp::Normalize, request_fingerprint: canonical_hash(&store, request) }
    );
}

#[test]
fn missing_term_is_malformed() {
    let store = TermStore::new();
    let edge = HyperEdge { nodes: vec![TermId(1), TermId(2)], predicate: predicates::REWRITE_EQUIVALENT };
    assert_eq!(hyper_edge_to_outer_candidate(&store, &edge), Err(AdmissionRejectReason::MalformedRelation));
}

#[test]
fn bad_arity_is_malformed() {
    let (store, left, _, _) = store_with_symbols();
    let edge = HyperEdge { nodes: vec![left], predicate: predicates::REWRITE_EQUIVALENT };
    assert_eq!(hyper_edge_to_outer_candidate(&store, &edge), Err(AdmissionRejectReason::MalformedRelation));
}

#[test]
fn unknown_predicate_is_malformed() {
    use athena_engine::reasoning::mgraph::PredicateId;

    let (store, only, _, _) = store_with_symbols();
    let edge = HyperEdge { nodes: vec![only], predicate: PredicateId(99) };
    assert_eq!(hyper_edge_to_outer_candidate(&store, &edge), Err(AdmissionRejectReason::MalformedRelation));
}
