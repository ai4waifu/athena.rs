//! 生产路径经 verified CFG 子集走 `athena-vm`（含 `LoadTerm` 原子项）。

use athena_engine::{
    Session,
    api::request::{AthenaRequest, ControlPlan},
    execution::execute_ir_request,
    runtime::CoverageStatus,
};
use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode};
use athena_types::ComputationStatus;

#[test]
fn execute_ir_request_not_uses_vm_path() {
    let mut session = Session::new();
    let t = session.builder().boolean(true, Default::default());
    let term = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Not), vec![t], Default::default());
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.status, ComputationStatus::Exact);
    assert_eq!(loaded.coverage, CoverageStatus::Full);
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Boolean(false))) => {}
        other => panic!("expected false, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_atom_term_uses_vm_load_term() {
    let mut session = Session::new();
    let term = session.builder().int(7, Default::default());
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.status, ComputationStatus::Exact);
    assert_eq!(loaded.coverage, CoverageStatus::Full);
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    assert_eq!(loaded.symbolic_term, Some(term));
    match session.arena.get(term) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(7) => {}
        other => panic!("expected int 7, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_hold_atom_uses_vm_load_term() {
    let mut session = Session::new();
    let inner = session.builder().int(5, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Hold),
        vec![inner],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    assert_eq!(loaded.symbolic_term, Some(term));
}

#[test]
fn execute_ir_request_add_integers_uses_vm_host() {
    let mut session = Session::new();
    let a = session.builder().int(2, Default::default());
    let b = session.builder().int(3, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Add),
        vec![a, b],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.status, ComputationStatus::Exact);
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5) => {}
        other => panic!("expected 5, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_multiply_integers_uses_vm_host() {
    let mut session = Session::new();
    let a = session.builder().int(4, Default::default());
    let b = session.builder().int(6, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Multiply),
        vec![a, b],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(24) => {}
        other => panic!("expected 24, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_subtract_integers_uses_vm_host() {
    let mut session = Session::new();
    let a = session.builder().int(9, Default::default());
    let b = session.builder().int(4, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Subtract),
        vec![a, b],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5) => {}
        other => panic!("expected 5, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_power_integers_uses_vm_host() {
    let mut session = Session::new();
    let a = session.builder().int(2, Default::default());
    let b = session.builder().int(5, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Power),
        vec![a, b],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(32) => {}
        other => panic!("expected 32, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_less_integers_uses_vm_host() {
    let mut session = Session::new();
    let a = session.builder().int(2, Default::default());
    let b = session.builder().int(5, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Less),
        vec![a, b],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Boolean(true))) => {}
        other => panic!("expected true, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_abs_integer_uses_vm_host() {
    let mut session = Session::new();
    let a = session.builder().int(-7, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Abs),
        vec![a],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(7) => {}
        other => panic!("expected 7, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_factorial_integer_uses_vm_host() {
    let mut session = Session::new();
    let a = session.builder().int(5, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Factorial),
        vec![a],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(120) => {}
        other => panic!("expected 120, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_define_and_read_uses_vm_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let sym_term = session.builder().symbol("x", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let value = session.builder().int(42, Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let define_id = execute_ir_request(&mut session, define).expect("define");
    let define_loaded = session.results.get(define_id).expect("define result");
    assert_eq!(
        define_loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    assert_eq!(session.defs.binding(symbol), Some(value));

    let read_id = execute_ir_request(&mut session, AthenaRequest::Term(sym_term)).expect("read");
    let read_loaded = session.results.get(read_id).expect("read result");
    assert_eq!(
        read_loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    assert_eq!(read_loaded.symbolic_term, Some(value));
}

#[test]
fn execute_ir_request_deferred_define_evaluates_on_vm_read() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(1, Default::default());
    let rhs = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Add),
        vec![a, b],
        Default::default(),
    );
    let sym_term = session.builder().symbol("a", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value: rhs,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::StoreResidualTerm,
    });
    execute_ir_request(&mut session, define).expect("define");
    assert!(session.defs.binding(symbol).is_none());
    assert_eq!(session.defs.residual_binding(symbol), Some(rhs));

    let read_id = execute_ir_request(&mut session, AthenaRequest::Term(sym_term)).expect("read");
    let read_loaded = session.results.get(read_id).expect("read result");
    assert_eq!(
        read_loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    let out = read_loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
        other => panic!("expected 2, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_domain_gcd_uses_vm_call_provider() {
    use athena_engine::api::request::DomainGoal;
    use athena_engine::domains::{dispatch::DomainRequest, number_theory::NumberTheoryRequest};
    use athena_numeric::Integer;

    let mut session = Session::new();
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(
        NumberTheoryRequest::Gcd {
            a: Integer::from_i64(12),
            b: Integer::from_i64(8),
        },
    )));
    let result_id = execute_ir_request(&mut session, request).expect("gcd");
    let loaded = session.results.get(result_id).expect("result");
    let provenance = loaded.provenance.as_ref().expect("provenance");
    assert_eq!(provenance.request_kind, "CallProvider");
    assert!(provenance.capability_fingerprint.is_some());
}

#[test]
fn execute_ir_request_local_scope_body_uses_vm() {
    let mut session = Session::new();
    let term = session.builder().int(11, Default::default());
    let request = AthenaRequest::Control(ControlPlan::LocalScope {
        body: Box::new(AthenaRequest::Term(term)),
    });
    let result_id = execute_ir_request(&mut session, request).expect("scope");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(
        loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    assert_eq!(loaded.symbolic_term, Some(term));
}

#[test]
fn execute_ir_request_local_scope_shadows_session_on_vm() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let sym_term = session.builder().symbol("s", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let global = session.builder().int(1, Default::default());
    let local = session.builder().int(2, Default::default());
    session.defs.write_binding(symbol, global);

    let request = AthenaRequest::Control(ControlPlan::LocalScope {
        body: Box::new(AthenaRequest::Control(ControlPlan::Sequence {
            steps: vec![
                AthenaRequest::Command(SessionCommand::Define {
                    symbol,
                    value: local,
                    kind: BindingKind::Session,
                    evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
                }),
                AthenaRequest::Term(sym_term),
            ],
        })),
    });
    let result_id = execute_ir_request(&mut session, request).expect("scope");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(
        loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    assert_eq!(loaded.symbolic_term, Some(local));
    assert_eq!(session.defs.binding(symbol), Some(global));
}

#[test]
fn execute_ir_request_ordered_collection_uses_vm() {
    use athena_types::CollectionKind;

    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let list = session
        .builder()
        .collection(CollectionKind::OrderedCollection, vec![a, b], Default::default());
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(list)).expect("list");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(
        loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection {
            kind: CollectionKind::OrderedCollection,
            elements,
        }) if elements.len() == 2 => {
            assert_eq!(elements[0], a);
            assert_eq!(elements[1], b);
        }
        other => panic!("expected ordered collection, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_join_lists_uses_vm_host() {
    use athena_types::CollectionKind;

    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let left = session
        .builder()
        .collection(CollectionKind::OrderedCollection, vec![a], Default::default());
    let right = session
        .builder()
        .collection(CollectionKind::OrderedCollection, vec![b, c], Default::default());
    let join = session.arena.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Join),
            arguments: vec![left, right],
        },
        TermNode::default_span(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(join)).expect("join");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(
        loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection {
            kind: CollectionKind::OrderedCollection,
            elements,
        }) if elements.as_slice() == [a, b, c] => {}
        other => panic!("expected Join result [1,2,3], got {other:?}"),
    }
}

#[test]
fn execute_ir_request_range_integer_uses_vm_host() {
    let mut session = Session::new();
    let n = session.builder().int(3, Default::default());
    let range = session.arena.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Range),
            arguments: vec![n],
        },
        TermNode::default_span(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(range)).expect("range");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(
        loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements, .. }) if elements.len() == 3 => {
            for (i, &el) in elements.iter().enumerate() {
                let expected = (i as i64) + 1;
                match session.arena.get(el) {
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(expected) => {}
                    other => panic!("expected Range element {expected}, got {other:?}"),
                }
            }
        }
        other => panic!("expected Range[3] length 3, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_size_list_uses_vm_host() {
    use athena_types::CollectionKind;
    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let list = session.arena.push(
        TermNode::Collection {
            kind: CollectionKind::OrderedCollection,
            elements: vec![a, b],
        },
        TermNode::default_span(),
    );
    let size = session.arena.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Size),
            arguments: vec![list],
        },
        TermNode::default_span(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(size)).expect("size");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(
        loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
            for (i, expected) in [1i64, 2].into_iter().enumerate() {
                match session.arena.get(items[i]) {
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(expected) => {}
                    other => panic!("expected Size dim {expected}, got {other:?}"),
                }
            }
        }
        other => panic!("expected Size == OrderedCollection[1,2], got {other:?}"),
    }
}

#[test]
fn execute_ir_request_sum_list_uses_vm_host() {
    use athena_types::CollectionKind;
    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let list = session.arena.push(
        TermNode::Collection {
            kind: CollectionKind::OrderedCollection,
            elements: vec![a, b, c],
        },
        TermNode::default_span(),
    );
    let sum = session.arena.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Sum),
            arguments: vec![list],
        },
        TermNode::default_span(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(sum)).expect("sum");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(
        loaded.provenance.as_ref().map(|p| p.request_kind),
        Some("ExecutionIR/athena-vm")
    );
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(6) => {}
        other => panic!("expected Sum=6, got {other:?}"),
    }
}
