//! Living `12` / `27` contract smoke — typed construction and evaluation only.
//!
//! These lib tests seed the neutral acceptance suite. Product crates still own
//! large integration coverage under `tests/main.rs`. This package must never
//! grow a `tests/` integration binary or dialect fixtures.

use athena_engine::api::AthenaRequest;
use athena_engine::api::request::SessionCommand;
use athena_engine::domains::calculus::{CalculusRequest, CalculusResult, CalculusValue, DerivativeOrder};
use athena_engine::domains::number_theory::{NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue};
use athena_engine::domains::{DomainRequest, DomainResult};
use athena_engine::reasoning::trs::TermPattern;
use athena_engine::runtime::values::arena::push_extension;
use athena_ir::{ApplicationHead, Atom, MathematicalConstant, SemanticOperator, TermNode, UnaryFunction};
use athena_numeric::Integer;
use athena_types::{BindingEvaluationPolicy, BindingKind, CollectionKind, ComputationStatus};

use crate::{assert_exact_integer, goal_request, SessionFixture};

#[test]
fn math_constant_pi_is_typed_atom_not_user_symbol() {
    let mut fx = SessionFixture::new();
    let pi = {
        let mut t = fx.terms();
        t.math_constant(MathematicalConstant::Pi)
    };
    let named = {
        let mut t = fx.terms();
        t.symbol("Pi")
    };
    assert!(!fx.structural_eq(pi, named));
    match fx.session().arena.get(pi) {
        Some(TermNode::Atom(Atom::Constant(MathematicalConstant::Pi))) => {}
        other => panic!("expected typed Pi constant, got {other:?}"),
    }
    match fx.session().arena.get(named) {
        Some(TermNode::Atom(Atom::Symbol(_))) => {}
        other => panic!("expected user symbol Pi, got {other:?}"),
    }
}

#[test]
fn math_constant_euler_number_is_not_user_symbol_e() {
    let mut fx = SessionFixture::new();
    let e = {
        let mut t = fx.terms();
        t.math_constant(MathematicalConstant::EulerNumber)
    };
    let named = {
        let mut t = fx.terms();
        t.symbol("E")
    };
    assert!(!fx.structural_eq(e, named));
    match fx.session().arena.get(e) {
        Some(TermNode::Atom(Atom::Constant(MathematicalConstant::EulerNumber))) => {}
        other => panic!("expected typed EulerNumber, got {other:?}"),
    }
}

#[test]
fn ordered_collection_carries_explicit_kind() {
    let mut fx = SessionFixture::new();
    let one = {
        let mut t = fx.terms();
        t.integer(1)
    };
    let two = {
        let mut t = fx.terms();
        t.integer(2)
    };
    let list = {
        let mut t = fx.terms();
        t.ordered([one, two])
    };
    match fx.session().arena.get(list) {
        Some(TermNode::Collection {
            kind: CollectionKind::OrderedCollection,
            elements,
        }) if elements.len() == 2 => {}
        other => panic!("expected OrderedCollection of length 2, got {other:?}"),
    }
}

#[test]
fn semantic_add_and_cos_pi_evaluate_exactly() {
    let mut fx = SessionFixture::new();
    let sum = {
        let mut t = fx.terms();
        let a = t.integer(2);
        let b = t.integer(3);
        t.add([a, b])
    };
    let sum_out = fx.evaluate_term(sum);
    assert_exact_integer(fx.session(), sum_out, 5);

    let cos_pi = {
        let mut t = fx.terms();
        let pi = t.math_constant(MathematicalConstant::Pi);
        t.unary_function(UnaryFunction::Cos, pi)
    };
    let cos_out = fx.evaluate_term(cos_pi);
    assert_exact_integer(fx.session(), cos_out, -1);
}

#[test]
fn register_compiled_rule_dispatches_extension_apply() {
    let mut fx = SessionFixture::new();
    let x_sym = {
        let mut t = fx.terms();
        t.intern("x")
    };
    let f_op = fx.session_mut().extensions.intern("f");
    let table = fx.session_mut().defs.alloc_dispatch_table();
    fx.session_mut().defs.bind_operator_table(f_op, table);
    let rhs = {
        let mut t = fx.terms();
        let x = t.symbol("x");
        let two = t.integer(2);
        t.power(x, two)
    };
    let pattern = TermPattern::Application {
        operator: ApplicationHead::Extension(f_op),
        arguments: vec![TermPattern::Bind {
            name: x_sym,
            inner: Box::new(TermPattern::Any),
        }],
    };
    let rule = fx.session_mut().compiled_rules.intern(pattern, rhs);
    fx.execute_request(AthenaRequest::Command(SessionCommand::RegisterRuleDispatch { table, rule }))
        .expect("register compiled rule");

    let call = {
        let three = {
            let mut t = fx.terms();
            t.integer(3)
        };
        push_extension(fx.session_mut(), f_op, vec![three])
    };
    let out = fx.evaluate_term(call);
    assert_exact_integer(fx.session(), out, 9);
}

#[test]
fn zero_ary_sin_maps_over_ordered_collection() {
    let mut fx = SessionFixture::new();
    let mapped = {
        let mut t = fx.terms();
        let sin = t.semantic(SemanticOperator::from_unary(UnaryFunction::Sin), []);
        let zero = t.integer(0);
        let list = t.ordered([zero]);
        t.semantic(SemanticOperator::Map, [sin, list])
    };
    let out = fx.evaluate_term(mapped);
    match fx.session().arena.get(out) {
        Some(TermNode::Collection {
            kind: CollectionKind::OrderedCollection,
            elements,
        }) if elements.len() == 1 => {
            assert_exact_integer(fx.session(), elements[0], 0);
        }
        other => panic!("expected OrderedCollection[0], got {other:?}"),
    }
}

#[test]
fn semantic_head_is_not_extension_operator_id() {
    let mut fx = SessionFixture::new();
    let add = {
        let mut t = fx.terms();
        let a = t.integer(1);
        let b = t.integer(2);
        t.add([a, b])
    };
    match fx.session().arena.get(add) {
        Some(TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Add),
            ..
        }) => {}
        other => panic!("expected Semantic(Add), got {other:?}"),
    }
    let ext = fx.session_mut().extensions.intern("user_plugin_op");
    let call = push_extension(fx.session_mut(), ext, vec![]);
    match fx.session().arena.get(call) {
        Some(TermNode::Application {
            head: ApplicationHead::Extension(id),
            ..
        }) => assert_eq!(*id, ext),
        other => panic!("expected Extension head, got {other:?}"),
    }
}

#[test]
fn domain_goal_derivative_uses_typed_calculus_request() {
    let mut fx = SessionFixture::new();
    let (expression, x) = {
        let mut t = fx.terms();
        let x = t.intern("x");
        let xs = t.symbol("x");
        (t.unary_function(UnaryFunction::Sin, xs), x)
    };
    let goal = fx.domain().derivative_first(expression, x);
    let athena_engine::api::DomainGoal::Dispatch(DomainRequest::Calculus(req)) = goal else {
        unreachable!()
    };
    assert!(matches!(
        req,
        CalculusRequest::Derivative {
            order: DerivativeOrder::First,
            ..
        }
    ));
    let engine = athena_engine::AthenaEngine::new();
    let result = engine
        .execute_domain(fx.session_mut(), DomainRequest::Calculus(req))
        .expect("sin'");
    match result {
        DomainResult::Calculus(CalculusResult::Exact {
            value: CalculusValue::Expression(term),
            ..
        })
        | DomainResult::Calculus(CalculusResult::Conditional {
            value: CalculusValue::Expression(term),
            ..
        }) => {
            assert!(fx.session().arena.get(term).is_some());
        }
        other => panic!("expected calculus expression, got {other:?}"),
    }
}

#[test]
fn session_binding_define_uses_typed_policy() {
    let mut fx = SessionFixture::new();
    let (symbol, value) = {
        let mut t = fx.terms();
        let sym_term = t.symbol("x");
        let value = t.integer(5);
        (sym_term, value)
    };
    let symbol = match fx.session().arena.get(symbol) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    fx.execute_request(AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    }))
    .expect("define");
    assert_eq!(fx.session().defs.binding(symbol), Some(value));
    let sum = {
        let mut t = fx.terms();
        let x = t.symbol("x");
        let one = t.integer(1);
        t.add([x, one])
    };
    let out = fx.evaluate_term(sum);
    assert_exact_integer(fx.session(), out, 6);
}

#[test]
fn number_theory_goal_request_preserves_typed_payload() {
    let mut fx = SessionFixture::new();
    let goal = fx.domain().dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
        a: Integer::from_i64(12),
        b: Integer::from_i64(8),
    }));
    let result_id = fx.execute_request(goal_request(goal)).expect("gcd goal");
    let loaded = fx.session().results.get(result_id).expect("payload");
    assert_eq!(loaded.status, ComputationStatus::Exact);
    let value_id = loaded.value.expect("value");
    match fx.session().values.get(value_id).expect("runtime") {
        athena_engine::runtime::RuntimeValue::Domain(DomainResult::NumberTheory(NumberTheoryResult::Exact {
            value: NumberTheoryValue::Integer(n),
        })) => assert_eq!(n, &Integer::from_i64(4)),
        other => panic!("expected gcd payload, got {other:?}"),
    }
}

#[test]
fn structural_term_equality_admits_into_exact_uf_and_proof_forest() {
    let mut fx = SessionFixture::new();
    let (left, right) = {
        let mut t = fx.terms();
        (t.integer(7), t.integer(7))
    };
    assert!(fx.session().arena.structural_eq(left, right));
    fx.session_mut()
        .admit_structural_term_equality(left, right)
        .expect("admit");
    let derived = &fx.session().mgraph.semantic.derived;
    assert_eq!(derived.exact_uf.find(left), derived.exact_uf.find(right));
    assert_eq!(derived.proof_forest.len(), 1);
}

#[test]
fn congruence_admit_keeps_classes_per_modulus() {
    let mut fx = SessionFixture::new();
    fx.session_mut().admit_congruence(7, 10, 20).expect("mod7");
    fx.session_mut().admit_congruence(11, 10, 30).expect("mod11");
    let congruence = &fx.session().mgraph.semantic.derived.congruence;
    assert_eq!(congruence.find(7, 10), congruence.find(7, 20));
    assert_ne!(congruence.find(7, 10), congruence.find(7, 30));
    assert_eq!(congruence.modulus_count(), 2);
}

#[test]
fn egraph_ruleset_saturation_emits_unverified_candidates() {
    use athena_rewriter::RuleSet;

    let mut fx = SessionFixture::new();
    let (zero, one) = {
        let mut t = fx.terms();
        (t.integer(0), t.integer(1))
    };
    let pattern = {
        let mut t = fx.terms();
        t.integer(0)
    };
    let mut rules = RuleSet::new();
    let rule_id = rules.push(pattern, one, Some("zero_to_one"));
    let report = fx.session_mut().run_egraph_saturation(&[zero], Some(&rules));
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].left_term, zero);
    assert_eq!(report.candidates[0].right_term, one);
    assert_eq!(report.candidates[0].rule, Some(rule_id));
    // Candidates stay outside M-Graph until explicit admit.
    assert_eq!(fx.session().mgraph.semantic.derived.proof_forest.len(), 0);
    let admitted = fx.session_mut().admit_structural_egraph_candidates(&report.candidates);
    assert!(admitted.is_empty());
}

#[test]
fn term_store_push_hash_conses_identical_integers() {
    let mut fx = SessionFixture::new();
    let (a, b, c) = {
        let mut t = fx.terms();
        (t.integer(5), t.integer(5), t.integer(6))
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn typed_egraph_saturation_binds_add_same() {
    use athena_engine::reasoning::egraph::TypedRuleSet;
    use athena_engine::reasoning::trs::TermPattern;
    use athena_ir::{ApplicationHead, SemanticOperator};

    let mut fx = SessionFixture::new();
    let (one, add, x_sym, x_term) = {
        let mut t = fx.terms();
        let one = t.integer(1);
        let add = t.add([one, one]);
        let x_sym = t.intern("x");
        let x_term = t.symbol("x");
        (one, add, x_sym, x_term)
    };
    let pattern = TermPattern::Application {
        operator: ApplicationHead::Semantic(SemanticOperator::Add),
        arguments: vec![
            TermPattern::Bind {
                name: x_sym,
                inner: Box::new(TermPattern::Any),
            },
            TermPattern::Bind {
                name: x_sym,
                inner: Box::new(TermPattern::Any),
            },
        ],
    };
    let mut rules = TypedRuleSet::new();
    rules.push(pattern, x_term, Some("add_same"));
    let report = fx.session_mut().run_egraph_saturation_typed(&[add], Some(&rules));
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].right_term, one);
    assert_eq!(fx.session().mgraph.semantic.derived.proof_forest.len(), 0);
}

#[test]
fn egraph_extract_smallest_ast_after_typed_saturation() {
    use athena_engine::reasoning::egraph::{ExtractionPreference, TypedRuleSet};
    use athena_engine::reasoning::trs::TermPattern;
    use athena_ir::{ApplicationHead, SemanticOperator};

    let mut fx = SessionFixture::new();
    let (one, add, x_sym, x_term) = {
        let mut t = fx.terms();
        let one = t.integer(1);
        let add = t.add([one, one]);
        let x_sym = t.intern("x");
        let x_term = t.symbol("x");
        (one, add, x_sym, x_term)
    };
    let pattern = TermPattern::Application {
        operator: ApplicationHead::Semantic(SemanticOperator::Add),
        arguments: vec![
            TermPattern::Bind {
                name: x_sym,
                inner: Box::new(TermPattern::Any),
            },
            TermPattern::Bind {
                name: x_sym,
                inner: Box::new(TermPattern::Any),
            },
        ],
    };
    let mut rules = TypedRuleSet::new();
    rules.push(pattern, x_term, Some("add_same"));
    let _ = fx.session_mut().run_egraph_saturation_typed(&[add], Some(&rules));
    let class = fx.session().egraph.class_of_term(add).expect("class");
    let extracted = fx
        .session()
        .extract_egraph_class(class, ExtractionPreference::SmallestAst)
        .expect("extract");
    assert_eq!(extracted, one);
}

#[test]
fn egraph_extract_result_cost_prefers_admitted_rep() {
    use athena_engine::reasoning::egraph::ExtractionPreference;
    use athena_engine::reasoning::mgraph::{
        AdmissionGate, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, VerificationPolicy,
    };

    let mut fx = SessionFixture::new();
    let (one, add) = {
        let mut t = fx.terms();
        let one = t.integer(1);
        let add = t.add([one, one]);
        (one, add)
    };
    {
        let session = fx.session_mut();
        let c_add = session.egraph.add_term(&session.arena, add).expect("add");
        let c_one = session.egraph.add_term(&session.arena, one).expect("one");
        session.egraph.union_classes(c_add, c_one);
    }
    AdmissionGate::admit_claim(
        &mut fx.session_mut().mgraph.semantic,
        Claim {
            proposition: Proposition::TermEquality { left: one, right: add },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: athena_engine::reasoning::egraph::EGRAPH_PROVIDER_ID,
                certificate: EvidenceCertificate::TestHarness,
                summary: "seed-one-add".into(),
            },
        },
        &VerificationPolicy::default(),
    )
    .expect("seed");
    let class = fx.session().egraph.class_of_term(add).expect("class");
    let (extracted, cost) = fx
        .session()
        .extract_egraph_class_with_cost(class, ExtractionPreference::ResultCost)
        .expect("extract");
    assert_eq!(extracted, one);
    assert!(cost.admitted_exact);
    assert_eq!(cost.ast_nodes, 1);
}

#[test]
fn application_congruence_rebuild_admits_from_exact_uf() {
    use athena_engine::reasoning::mgraph::{
        AdmissionGate, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, VerificationPolicy,
    };

    let mut fx = SessionFixture::new();
    let (x, y, add_xz, add_yz) = {
        let mut t = fx.terms();
        let x = t.symbol("x");
        let y = t.symbol("y");
        let z = t.symbol("z");
        let add_xz = t.add([x, z]);
        let add_yz = t.add([y, z]);
        (x, y, add_xz, add_yz)
    };
    AdmissionGate::admit_claim(
        &mut fx.session_mut().mgraph.semantic,
        Claim {
            proposition: Proposition::TermEquality { left: x, right: y },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: athena_engine::reasoning::egraph::EGRAPH_PROVIDER_ID,
                certificate: EvidenceCertificate::TestHarness,
                summary: "seed-xy".into(),
            },
        },
        &VerificationPolicy::default(),
    )
    .expect("seed");
    {
        let session = fx.session_mut();
        session.egraph.add_term(&session.arena, add_xz).expect("add xz");
        session.egraph.add_term(&session.arena, add_yz).expect("add yz");
    }
    let admitted = fx.session_mut().rebuild_and_admit_application_congruence(8);
    assert_eq!(admitted.len(), 1);
    assert!(admitted[0].is_ok());
    let derived = &fx.session().mgraph.semantic.derived;
    assert_eq!(derived.exact_uf.find(add_xz), derived.exact_uf.find(add_yz));
    assert!(derived.proof_forest.len() >= 2);
}

#[test]
fn typed_egraph_admit_pipeline_closes_application_congruence() {
    use athena_engine::reasoning::mgraph::{
        AdmissionGate, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, VerificationPolicy,
    };

    let mut fx = SessionFixture::new();
    let (x, y, add_xz, add_yz) = {
        let mut t = fx.terms();
        let x = t.symbol("x");
        let y = t.symbol("y");
        let z = t.symbol("z");
        (x, y, t.add([x, z]), t.add([y, z]))
    };
    AdmissionGate::admit_claim(
        &mut fx.session_mut().mgraph.semantic,
        Claim {
            proposition: Proposition::TermEquality { left: x, right: y },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: athena_engine::reasoning::egraph::EGRAPH_PROVIDER_ID,
                certificate: EvidenceCertificate::TestHarness,
                summary: "seed-xy".into(),
            },
        },
        &VerificationPolicy::default(),
    )
    .expect("seed");
    let report = fx
        .session_mut()
        .run_typed_egraph_admit_pipeline(&[add_xz, add_yz], None, 8);
    assert!(report.structural_admitted.is_empty());
    assert!(report.rewrite_admitted.is_empty());
    assert_eq!(report.congruence_admitted.len(), 1);
    assert!(report.congruence_admitted[0].is_ok());
    let derived = &fx.session().mgraph.semantic.derived;
    assert_eq!(derived.exact_uf.find(add_xz), derived.exact_uf.find(add_yz));
}

#[test]
fn typed_egraph_admit_pipeline_replays_rewrite_rules() {
    use athena_engine::reasoning::egraph::TypedRuleSet;
    use athena_engine::reasoning::trs::TermPattern;
    use athena_ir::{ApplicationHead, SemanticOperator};

    let mut fx = SessionFixture::new();
    let (one, add, x_sym, x_term) = {
        let mut t = fx.terms();
        let one = t.integer(1);
        let add = t.add([one, one]);
        let x_sym = t.intern("x");
        let x_term = t.symbol("x");
        (one, add, x_sym, x_term)
    };
    let pattern = TermPattern::Application {
        operator: ApplicationHead::Semantic(SemanticOperator::Add),
        arguments: vec![
            TermPattern::Bind {
                name: x_sym,
                inner: Box::new(TermPattern::Any),
            },
            TermPattern::Bind {
                name: x_sym,
                inner: Box::new(TermPattern::Any),
            },
        ],
    };
    let mut rules = TypedRuleSet::new();
    rules.push(pattern, x_term, Some("add_same"));
    let report = fx
        .session_mut()
        .run_typed_egraph_admit_pipeline(&[add], Some(&rules), 8);
    assert_eq!(report.saturation.candidates.len(), 1);
    assert!(report.structural_admitted.is_empty());
    assert_eq!(report.rewrite_admitted.len(), 1);
    assert!(report.rewrite_admitted[0].is_ok());
    let derived = &fx.session().mgraph.semantic.derived;
    assert_eq!(derived.exact_uf.find(add), derived.exact_uf.find(one));
}

#[test]
fn predicate_registry_and_hyper_edge_staging_are_typed() {
    use athena_engine::reasoning::mgraph::{
        CalculusRelationKind, Guarantee, HyperEdge, Proposition, arity_ok, descriptor,
        hyper_edge_to_outer_candidate, predicates,
    };
    use athena_ir::canonical_hash;

    let desc = descriptor(predicates::REWRITE_EQUIVALENT).expect("registered");
    assert!(arity_ok(predicates::REWRITE_EQUIVALENT, 2));
    assert_eq!(desc.subject_arity, 2..=2);

    let mut fx = SessionFixture::new();
    let (left, right, expr, var, result) = {
        let mut t = fx.terms();
        (t.symbol("l"), t.symbol("r"), t.symbol("e"), t.symbol("v"), t.symbol("out"))
    };

    let edge = HyperEdge {
        nodes: vec![left, right],
        predicate: predicates::REWRITE_EQUIVALENT,
    };
    let outer = hyper_edge_to_outer_candidate(&fx.session().arena, &edge).expect("stage");
    assert_eq!(outer.claim.guarantee, Guarantee::Candidate);
    assert_eq!(
        outer.claim.proposition,
        Proposition::TermEquality { left, right }
    );

    let calc = HyperEdge {
        nodes: vec![expr, var, result],
        predicate: predicates::DERIVATIVE_OF,
    };
    let calc_outer = hyper_edge_to_outer_candidate(&fx.session().arena, &calc).expect("calculus");
    match calc_outer.claim.proposition {
        Proposition::CalculusRelation {
            kind,
            expression_fingerprint,
            variable_fingerprint,
            result_term,
        } => {
            assert_eq!(kind, CalculusRelationKind::DerivativeOf);
            assert_eq!(expression_fingerprint, canonical_hash(&fx.session().arena, expr));
            assert_eq!(variable_fingerprint, canonical_hash(&fx.session().arena, var));
            assert_eq!(result_term, result);
        }
        other => panic!("expected CalculusRelation, got {other:?}"),
    }

    assert_eq!(fx.session().mgraph.semantic.derived.exact_uf.union_count(), 0);
}

#[test]
fn typed_rewrite_replay_admits_add_same_into_exact_uf() {
    use athena_engine::reasoning::egraph::TypedRuleSet;
    use athena_engine::reasoning::trs::TermPattern;
    use athena_ir::{ApplicationHead, SemanticOperator};

    let mut fx = SessionFixture::new();
    let (one, add, x_sym, x_term) = {
        let mut t = fx.terms();
        let one = t.integer(1);
        let add = t.add([one, one]);
        let x_sym = t.intern("x");
        let x_term = t.symbol("x");
        (one, add, x_sym, x_term)
    };
    let pattern = TermPattern::Application {
        operator: ApplicationHead::Semantic(SemanticOperator::Add),
        arguments: vec![
            TermPattern::Bind {
                name: x_sym,
                inner: Box::new(TermPattern::Any),
            },
            TermPattern::Bind {
                name: x_sym,
                inner: Box::new(TermPattern::Any),
            },
        ],
    };
    let mut rules = TypedRuleSet::new();
    rules.push(pattern, x_term, Some("add_same"));
    let report = fx.session_mut().run_egraph_saturation_typed(&[add], Some(&rules));
    assert_eq!(report.candidates.len(), 1);
    let admitted = fx
        .session_mut()
        .admit_typed_rewrite_egraph_candidates(&rules, &report.candidates);
    assert_eq!(admitted.len(), 1);
    assert!(admitted[0].is_ok());
    let derived = &fx.session().mgraph.semantic.derived;
    assert_eq!(derived.exact_uf.find(add), derived.exact_uf.find(one));
}

#[test]
fn mgraph_closure_materializes_transitivity_in_proof_forest() {
    use athena_engine::reasoning::mgraph::{
        AdmissionGate, Claim, ClosureLimits, ClosureStopReason, Evidence, EvidenceCertificate, Guarantee,
        ProofStepKind, Proposition, Scope, VerificationPolicy,
    };

    let mut fx = SessionFixture::new();
    let (a, b, c) = {
        let mut t = fx.terms();
        (t.symbol("a"), t.symbol("b"), t.symbol("c"))
    };
    for (left, right) in [(a, b), (b, c)] {
        AdmissionGate::admit_claim(
            &mut fx.session_mut().mgraph.semantic,
            Claim {
                proposition: Proposition::TermEquality { left, right },
                scope: Scope::Unconditional,
                guarantee: Guarantee::ProvenExact,
                evidence: Evidence::TrustedKernel {
                    provider: athena_engine::reasoning::egraph::EGRAPH_PROVIDER_ID,
                    certificate: EvidenceCertificate::TestHarness,
                    summary: "seed".into(),
                },
            },
            &VerificationPolicy::default(),
        )
        .expect("seed");
    }
    assert_eq!(fx.session().mgraph.semantic.derived.proof_forest.len(), 2);
    let result = fx.session_mut().run_mgraph_closure(ClosureLimits::default());
    assert_eq!(result.stop, ClosureStopReason::Saturated);
    assert!(result.steps_applied >= 1);
    assert!(fx.session().mgraph.semantic.derived.proof_forest.edges().iter().any(|e| {
        e.step_kind == ProofStepKind::Transitivity
            && ((e.left == a && e.right == c) || (e.left == c && e.right == a))
    }));
}

#[test]
fn mgraph_closure_drains_hyper_edges_without_admit() {
    use athena_engine::reasoning::mgraph::{ClosureLimits, Guarantee, HyperEdge, Proposition, predicates};

    let mut fx = SessionFixture::new();
    let (left, right) = {
        let mut t = fx.terms();
        (t.symbol("x"), t.symbol("y"))
    };
    fx.session_mut().mgraph.operational.hyper_edges.push(HyperEdge {
        nodes: vec![left, right],
        predicate: predicates::REWRITE_EQUIVALENT,
    });
    let result = fx.session_mut().run_mgraph_closure(ClosureLimits::default());
    assert_eq!(result.hyper_edges_staged, 1);
    assert!(fx.session().mgraph.operational.hyper_edges.is_empty());
    assert_eq!(fx.session().mgraph.operational.outer_candidates.len(), 1);
    assert_eq!(
        fx.session().mgraph.operational.outer_candidates[0].claim.guarantee,
        Guarantee::Candidate
    );
    assert_eq!(
        fx.session().mgraph.operational.outer_candidates[0].claim.proposition,
        Proposition::TermEquality { left, right }
    );
    assert_eq!(fx.session().mgraph.semantic.relation_count(), 0);
}

#[test]
fn mgraph_outer_pool_admits_only_structural_equality() {
    use athena_engine::reasoning::mgraph::{Guarantee, HyperEdge, Proposition, predicates};
    use athena_ir::{Atom, TermNode};
    use athena_types::SourceSpan;

    let mut fx = SessionFixture::new();
    let (same_a, same_b, left, right) = {
        let session = fx.session_mut();
        let span = SourceSpan::default();
        let same_a = session.arena.push(
            TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(3))),
            span,
        );
        let same_b = session.arena.push(
            TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(3))),
            span,
        );
        let x = session.arena.symbols_mut().intern("p");
        let y = session.arena.symbols_mut().intern("q");
        let left = session.arena.push(TermNode::Atom(Atom::Symbol(x)), span);
        let right = session.arena.push(TermNode::Atom(Atom::Symbol(y)), span);
        (same_a, same_b, left, right)
    };
    assert_eq!(same_a, same_b);
    fx.session_mut().mgraph.operational.hyper_edges.push(HyperEdge {
        nodes: vec![same_a, same_b],
        predicate: predicates::REWRITE_EQUIVALENT,
    });
    fx.session_mut().mgraph.operational.hyper_edges.push(HyperEdge {
        nodes: vec![left, right],
        predicate: predicates::REWRITE_EQUIVALENT,
    });
    assert_eq!(fx.session_mut().drain_mgraph_hyper_edges().staged, 2);
    let report = fx.session_mut().admit_mgraph_outer_pool_if_structural();
    assert_eq!(report.admitted, 1);
    assert_eq!(report.retained, 1);
    assert_eq!(fx.session().mgraph.operational.outer_candidates.len(), 1);
    assert_eq!(
        fx.session().mgraph.operational.outer_candidates[0].claim.proposition,
        Proposition::TermEquality { left, right }
    );
    assert_eq!(
        fx.session().mgraph.operational.outer_candidates[0].claim.guarantee,
        Guarantee::Candidate
    );
    assert_eq!(fx.session().mgraph.semantic.relation_count(), 1);
}

#[test]
fn mgraph_admit_wakes_registered_obligation() {
    use athena_engine::reasoning::mgraph::{
        Claim, Evidence, EvidenceCertificate, Guarantee, POLYNOMIAL_PROVIDER_ID, ProofObligation, Proposition, Scope,
        ScopeRef, predicates,
    };

    let mut fx = SessionFixture::new();
    fx.session_mut().register_mgraph_obligation(ProofObligation {
        predicate: predicates::POLYNOMIAL_RESULT,
        scope: ScopeRef::UNCONDITIONAL,
        known_objects: vec![],
    });
    let (_, wake) = fx
        .session_mut()
        .admit_mgraph_claim_with_wake(Claim {
            proposition: Proposition::PolynomialResult {
                operation: athena_engine::domains::polynomial::PolynomialCacheOp::Add,
                request_fingerprint: 101,
            },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: POLYNOMIAL_PROVIDER_ID,
                certificate: EvidenceCertificate::TestHarness,
                summary: "wake".into(),
            },
        })
        .expect("admit");
    assert_eq!(wake.wakes.len(), 1);
    assert!(fx.session().mgraph.operational.obligation_index.is_empty());
}

#[test]
fn mgraph_schedule_wakes_queues_need_computation() {
    use athena_engine::domains::planner::{DomainPlan, PlanStep};
    use athena_engine::reasoning::mgraph::{
        MGraphView, ProofObligation, Reflection, ReflectorWake, ScopeRef, SemanticReflector, FactId, predicates,
    };

    struct AlwaysCompute;
    impl SemanticReflector for AlwaysCompute {
        fn reflect(&self, _obligation: &ProofObligation, _view: &MGraphView<'_>) -> Reflection {
            Reflection::NeedComputation {
                plan: DomainPlan {
                    steps: vec![PlanStep::CallDomainProvider, PlanStep::MaterializeResult],
                },
            }
        }
    }

    let mut fx = SessionFixture::new();
    let wakes = [ReflectorWake {
        obligation: ProofObligation {
            predicate: predicates::POLYNOMIAL_RESULT,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![],
        },
        relation: FactId(0),
    }];
    let report = fx
        .session_mut()
        .schedule_mgraph_reflector_wakes(&wakes, &AlwaysCompute);
    assert_eq!(report.need_computation, 1);
    assert_eq!(fx.session().mgraph.operational.pending_plans.len(), 1);
}
