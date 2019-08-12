use super::super::*;
use crate::{execution::reference::ReferenceExecutor, runtime::session::Session};
use athena_types::ComputationStatus;

#[test]
fn compile_and_execute_boolean_branch() {
    let mut session = Session::new();
    let cond = session.builder().boolean(true, Default::default());
    let then_term = session.builder().int(1, Default::default());
    let else_term = session.builder().int(0, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Branch {
        condition: cond,
        then_branch: Box::new(AthenaRequest::Term(then_term)),
        else_branch: Some(Box::new(AthenaRequest::Term(else_term))),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("branch");
    assert_eq!(module.regions[0].blocks.len(), 3);
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(then_term));
    assert_eq!(loaded.status, ComputationStatus::Exact);
}

#[test]
fn compile_and_execute_define_write_binding() {
    use athena_types::{BindingEvaluationPolicy, BindingKind};
    use crate::api::request::SessionCommand;

    let mut session = Session::new();
    let sym_term = session.builder().symbol("x", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let value = session.builder().int(42, Default::default());
    let request = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("define");
    assert!(!module.effect_edges.is_empty());
    ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    assert_eq!(session.defs.binding(symbol), Some(value));
}

#[test]
fn compile_and_execute_define_deferred_evaluates_on_read() {
    use athena_types::{BindingEvaluationPolicy, BindingKind};
    use crate::api::request::SessionCommand;

    let mut session = Session::new();
    let plus = session.operators.intern("Plus");
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(1, Default::default());
    let rhs = session.builder().application(plus, vec![a, b], Default::default());
    let sym_term = session.builder().symbol("a", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let request = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value: rhs,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::StoreResidualTerm,
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("define residual");
    ReferenceExecutor::new().execute(&mut session, &module, None).expect("define exec");
    assert!(session.defs.binding(symbol).is_none());
    assert_eq!(session.defs.residual_binding(symbol), Some(rhs));

    let read_module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(sym_term)).expect("read");
    let result_id = ReferenceExecutor::new().execute(&mut session, &read_module, None).expect("read exec");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
        other => panic!("expected residual Plus[1,1] == 2, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_define_then_read_binding() {
    use athena_types::{BindingEvaluationPolicy, BindingKind};
    use crate::api::request::SessionCommand;

    let mut session = Session::new();
    let sym_term = session.builder().symbol("y", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let value = session.builder().int(7, Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let define_module = ExecutionCompiler::new().compile(&mut session, &define).expect("define");
    ReferenceExecutor::new().execute(&mut session, &define_module, None).expect("define exec");

    let read = AthenaRequest::Term(sym_term);
    let read_module = ExecutionCompiler::new().compile(&mut session, &read).expect("read");
    let result_id = ReferenceExecutor::new().execute(&mut session, &read_module, None).expect("read exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(value));
}

#[test]
fn compile_and_execute_sequence_define_read_clear() {
    use athena_types::{BindingEvaluationPolicy, BindingKind};
    use crate::api::request::SessionCommand;

    let mut session = Session::new();
    let sym_term = session.builder().symbol("z", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let value = session.builder().int(5, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    }),
            AthenaRequest::Term(sym_term),
            AthenaRequest::Command(SessionCommand::ClearDefinition { symbol }),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("sequence");
    assert_eq!(module.regions[0].blocks.len(), 3);
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    // Last step clears; result is Unit → Null term.
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Null)) => {}
        other => panic!("expected Null after clear, got {other:?}"),
    }
    assert!(session.defs.binding(symbol).is_none());
}

#[test]
fn compile_and_execute_counted_loop_unroll() {
    let mut session = Session::new();
    let var = session.builder().symbol("i", Default::default());
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let iter = session.builder().list(vec![a, b, c], Default::default());
    let request =
        AthenaRequest::Control(ControlPlan::CountedLoop { variable: var, iterator: iter, body: Box::new(AthenaRequest::Term(var)) });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("counted");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(c));
    let symbol = match session.arena.get(var) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    assert_eq!(session.defs.binding(symbol), Some(c));
}

#[test]
fn compile_and_execute_iterate_collects_collection() {
    use athena_types::BindingEvaluationPolicy;

    let mut session = Session::new();
    let var = session.builder().symbol("i", Default::default());
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let range = session.builder().list(vec![a, b, c], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Iterate {
        binder: var,
        range,
        body: Box::new(AthenaRequest::Term(var)),
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("iterate");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(term) {
        Some(TermNode::Collection { elements, .. }) => {
            assert_eq!(elements.len(), 3);
        }
        other => panic!("expected collection, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_term_counted_loop_span() {
    let mut session = Session::new();
    let var = session.builder().symbol("i", Default::default());
    let one = session.builder().int(1, Default::default());
    let three = session.builder().int(3, Default::default());
    let span = session.operators.intern("Span");
    let loop_op = session.operators.intern("CountedLoop");
    let iter = session.builder().application(span, vec![one, three], Default::default());
    let term = session.builder().application(loop_op, vec![var, iter, var], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("counted span");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3) => {}
        other => panic!("expected CountedLoop[i, Span[1,3], i] == 3, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_loop_while_false() {
    let mut session = Session::new();
    let cond = session.builder().boolean(false, Default::default());
    let body = session.builder().int(1, Default::default());
    let request = AthenaRequest::Control(ControlPlan::LoopWhile { condition: cond, body: Box::new(AthenaRequest::Term(body)) });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("loop");
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::BudgetCheck)));
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Null)) => {}
        other => panic!("expected Unit/Null after zero-trip loop, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_term_loop_while_zero() {
    let mut session = Session::new();
    let zero = session.builder().int(0, Default::default());
    let body = session.builder().int(1, Default::default());
    let head = session.operators.intern("LoopWhile");
    let term = session.builder().application(head, vec![zero, body], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("loop term");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Null)) => {}
        other => panic!("expected Null after LoopWhile[0,1], got {other:?}"),
    }
}

#[test]
fn compile_and_execute_goal_call_provider_dispatches_domain() {
    use crate::{
        api::request::DomainGoal,
        domains::{dispatch::DomainRequest, number_theory::NumberTheoryRequest},
        execution::execute_ir_request,
        runtime::values::RuntimeValue,
    };
    use athena_numeric::Integer;

    let mut session = Session::new();
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
        a: Integer::from_i64(12),
        b: Integer::from_i64(8),
    })));
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("goal");
    assert_eq!(module.provider_calls.len(), 1);
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::CallProvider)));
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::PublishResult)));
    let result_id = execute_ir_request(&mut session, request).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.coverage, crate::runtime::results::CoverageStatus::Full);
    let value_id = loaded.value.expect("value");
    match session.values.get(value_id).expect("runtime") {
        RuntimeValue::Domain(crate::domains::dispatch::DomainResult::NumberTheory(
            crate::domains::number_theory::NumberTheoryResult::Exact {
                value: crate::domains::number_theory::NumberTheoryValue::Integer(n),
            },
        )) => assert_eq!(n, &Integer::from_i64(4)),
        other => panic!("expected NumberTheory Exact Integer gcd, got {other:?}"),
    }
}

#[test]
fn call_provider_without_domain_stays_unsupported() {
    use crate::{
        api::request::DomainGoal,
        domains::{dispatch::DomainRequest, number_theory::NumberTheoryRequest},
    };
    use athena_numeric::Integer;

    let mut session = Session::new();
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
        a: Integer::from_i64(12),
        b: Integer::from_i64(8),
    })));
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("goal");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.coverage, crate::runtime::results::CoverageStatus::Unsupported);
    assert!(!loaded.diagnostics.is_empty());
}

#[test]
fn compile_and_execute_recover_success_body() {
    let mut session = Session::new();
    let body = session.builder().int(8, Default::default());
    let handler = session.builder().int(9, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Recover {
        body: Box::new(AthenaRequest::Term(body)),
        handler: Box::new(AthenaRequest::Term(handler)),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("recover");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(body));
}

#[test]
fn compile_and_execute_term_recover_error_and_success() {
    let mut session = Session::new();
    let recover = session.operators.intern("Recover");
    let error = session.operators.intern("error");
    let msg = session.builder().string("e", Default::default());
    let err_body = session.builder().application(error, vec![msg], Default::default());
    let one = session.builder().int(1, Default::default());
    let err_term = session.builder().application(recover, vec![err_body, one], Default::default());
    let err_mod = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(err_term)).expect("recover err");
    let err_id = ReferenceExecutor::new().execute(&mut session, &err_mod, None).expect("err exec");
    let err_out = session.results.get(err_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(err_out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1) => {}
        other => panic!("expected Recover[error,1] == 1, got {other:?}"),
    }

    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let ok_term = session.builder().application(recover, vec![two, three], Default::default());
    let ok_mod = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(ok_term)).expect("recover ok");
    let ok_id = ReferenceExecutor::new().execute(&mut session, &ok_mod, None).expect("ok exec");
    let ok_out = session.results.get(ok_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(ok_out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
        other => panic!("expected Recover[2,3] == 2, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_cond_second_arm() {
    let mut session = Session::new();
    let c0 = session.builder().boolean(false, Default::default());
    let c1 = session.builder().boolean(true, Default::default());
    let a0 = session.builder().int(10, Default::default());
    let a1 = session.builder().int(20, Default::default());
    let otherwise = session.builder().int(30, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Cond {
        arms: vec![(c0, Box::new(AthenaRequest::Term(a0))), (c1, Box::new(AthenaRequest::Term(a1)))],
        otherwise: Some(Box::new(AthenaRequest::Term(otherwise))),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("cond");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(a1));
}

#[test]
fn compile_and_execute_local_scope_body() {
    let mut session = Session::new();
    let term = session.builder().int(11, Default::default());
    let request = AthenaRequest::Control(ControlPlan::LocalScope { body: Box::new(AthenaRequest::Term(term)) });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("scope");
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::EnterScope)));
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::ExitScope)));
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(term));
}

#[test]
fn compile_and_execute_local_scope_shadows_session() {
    use athena_types::{BindingEvaluationPolicy, BindingKind};
    use crate::api::request::SessionCommand;

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
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("scope");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(local));
    // Session Own unchanged after local scope exits.
    assert_eq!(session.defs.binding(symbol), Some(global));
}

#[test]
fn compile_and_execute_term_local_scope_with_define() {
    let mut session = Session::new();
    let define = session.operators.intern("Define");
    let plus = session.operators.intern("Plus");
    let scope = session.operators.intern("LocalScope");
    let x = session.builder().symbol("x", Default::default());
    let one = session.builder().int(1, Default::default());
    let def = session.builder().application(define, vec![x, one], Default::default());
    let locals = session.builder().list(vec![def], Default::default());
    let one2 = session.builder().int(1, Default::default());
    let body = session.builder().application(plus, vec![x, one2], Default::default());
    let term = session.builder().application(scope, vec![locals, body], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("local scope");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
        other => panic!("expected LocalScope Define body == 2, got {other:?}"),
    }
    let symbol = match session.arena.get(x) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    assert!(session.defs.binding(symbol).is_none());
}

#[test]
fn compile_and_execute_term_lexical_scope_bare_unique() {
    let mut session = Session::new();
    let scope = session.operators.intern("LexicalScope");
    let x = session.builder().symbol("x", Default::default());
    let locals = session.builder().list(vec![x], Default::default());
    let term = session.builder().application(scope, vec![locals, x], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("lexical");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Symbol(sym))) => {
            let name = session.arena.symbols().resolve(*sym).unwrap_or("");
            assert!(name.starts_with("x$"), "got {name}");
        }
        other => panic!("expected unique x$N, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_boolean_not_and() {
    let mut session = Session::new();
    let t = session.builder().boolean(true, Default::default());
    let f = session.builder().boolean(false, Default::default());
    let and = session.operators.intern("And");
    let not = session.operators.intern("Not");
    let and_term = session.builder().application(and, vec![t, f], Default::default());
    let term = session.builder().application(not, vec![and_term], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("bool ops");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Boolean(true))) => {}
        other => panic!("expected Not[And[True,False]] == True, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_term_if() {
    let mut session = Session::new();
    let cond = session.builder().boolean(true, Default::default());
    let then_term = session.builder().int(11, Default::default());
    let else_term = session.builder().int(22, Default::default());
    let if_op = session.operators.intern("If");
    let term = session.builder().application(if_op, vec![cond, then_term, else_term], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("if");
    assert_eq!(module.regions[0].blocks.len(), 3);
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(then_term));
}

#[test]
fn compile_and_execute_sequence_and_hold() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let seq = session.operators.intern("Sequence");
    let term = session.builder().application(seq, vec![one, two, three], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("sequence");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(three));

    let plus = session.operators.intern("Plus");
    let hold = session.operators.intern("Hold");
    let inner = session.builder().application(plus, vec![one, one], Default::default());
    let held = session.builder().application(hold, vec![inner], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(held)).expect("hold");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Application { head, arguments })
            if session.operators.name(*head) == Some("Hold")
                && arguments.len() == 1
                && session.arena.structural_eq(arguments[0], inner) => {}
        other => panic!("expected Hold[Plus[1,1]] unevaluated, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_part_end_and_cond() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let list = session.builder().list(vec![one, two, three], Default::default());
    let end = session.builder().symbol("End", Default::default());
    let part = session.operators.intern("Part");
    let term = session.builder().application(part, vec![list, end], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("part end");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(three));

    let fals = session.builder().symbol("False", Default::default());
    let tru = session.builder().symbol("True", Default::default());
    let cond = session.operators.intern("Cond");
    let term = session.builder().application(cond, vec![fals, one, tru, two, tru, three], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("cond");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(two));
}

#[test]
fn compile_and_execute_term_define_in_sequence() {
    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let five = session.builder().int(5, Default::default());
    let one = session.builder().int(1, Default::default());
    let define = session.operators.intern("Define");
    let plus = session.operators.intern("Plus");
    let seq = session.operators.intern("Sequence");
    let def = session.builder().application(define, vec![x, five], Default::default());
    let use_x = session.builder().application(plus, vec![x, one], Default::default());
    let term = session.builder().application(seq, vec![def, use_x], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("define seq");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(6) => {}
        other => panic!("expected Sequence[Define[x,5], Plus[x,1]] == 6, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_runtime_branch() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let seven = session.builder().int(7, Default::default());
    let eight = session.builder().int(8, Default::default());
    let equal = session.operators.intern("Equal");
    let branch = session.operators.intern("Branch");
    let cond = session.builder().application(equal, vec![one, one], Default::default());
    let term = session.builder().application(branch, vec![cond, seven, eight], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("branch");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(seven));

    let fals = session.builder().symbol("False", Default::default());
    let term = session.builder().application(branch, vec![fals, seven, eight], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("branch false");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(eight));
}

#[test]
fn compile_and_execute_sameq_and_trueq() {
    let mut session = Session::new();
    let t = session.builder().boolean(true, Default::default());
    let f = session.builder().boolean(false, Default::default());
    let same = session.operators.intern("SameQ");
    let true_q = session.operators.intern("TrueQ");
    let same_term = session.builder().application(same, vec![t, f], Default::default());
    let term = session.builder().application(true_q, vec![same_term], Default::default());
    // TrueQ[SameQ[True,False]] == TrueQ[False] == False
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("sameq");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Boolean(false))) => {}
        other => panic!("expected False, got {other:?}"),
    }

    let a = session.builder().int(3, Default::default());
    let b = session.builder().int(3, Default::default());
    let eq = session.operators.intern("Equal");
    let eq_term = session.builder().application(eq, vec![a, b], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(eq_term)).expect("equal");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Boolean(true))) => {}
        other => panic!("expected Equal[3,3] == True, got {other:?}"),
    }
}
