//! `ExecutionIR` compile-time contracts: effects, fingerprints, and re-verify.

use athena_engine::{
    api::request::{AthenaRequest, ControlPlan, DomainGoal, SessionCommand},
    domains::{dispatch::DomainRequest, number_theory::NumberTheoryRequest},
    execution::{
        compiler::ExecutionCompiler,
        ir::{EffectKind, ModuleFingerprint, OperationKind, verify_module},
    },
    runtime::Session,
};
use athena_numeric::Integer;
use athena_types::{BindingEvaluationPolicy, BindingKind};

fn compile(session: &mut Session, request: AthenaRequest) -> athena_engine::execution::ir::ExecutionModule {
    ExecutionCompiler::new().compile(session, &request).expect("compile")
}

#[test]
fn compiled_module_fingerprint_matches_recompute() {
    let mut session = Session::new();
    let term = session.builder().int(1, Default::default());
    let module = compile(&mut session, AthenaRequest::Term(term));
    assert_eq!(module.fingerprint, ModuleFingerprint::of_module(&module));
    verify_module(&module).expect("re-verify");
}

#[test]
fn define_emits_write_binding_effect_chain() {
    let mut session = Session::new();
    let sym_term = session.builder().symbol("x", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let value = session.builder().int(3, Default::default());
    let module = compile(
        &mut session,
        AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    }),
    );
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::WriteBinding)));
    assert!(module.regions[0].blocks.iter().any(|b| {
        b.operations.iter().any(|op| matches!(op.kind, OperationKind::WriteBinding { .. }) && op.effect_in.is_some() && op.effect_out.is_some())
    }));
    verify_module(&module).expect("define effects");
}

#[test]
fn branch_module_has_three_blocks_and_stable_fingerprint() {
    let mut session = Session::new();
    let cond = session.builder().boolean(true, Default::default());
    let then_term = session.builder().int(1, Default::default());
    let else_term = session.builder().int(0, Default::default());
    let module = compile(
        &mut session,
        AthenaRequest::Control(ControlPlan::Branch {
            condition: cond,
            then_branch: Box::new(AthenaRequest::Term(then_term)),
            else_branch: Some(Box::new(AthenaRequest::Term(else_term))),
        }),
    );
    assert_eq!(module.regions[0].blocks.len(), 3);
    let again = ModuleFingerprint::of_module(&module);
    assert_eq!(module.fingerprint, again);
    verify_module(&module).expect("branch");
}

#[test]
fn loop_while_emits_budget_check_effect() {
    let mut session = Session::new();
    let cond = session.builder().boolean(false, Default::default());
    let body = session.builder().int(1, Default::default());
    let module = compile(
        &mut session,
        AthenaRequest::Control(ControlPlan::LoopWhile { condition: cond, body: Box::new(AthenaRequest::Term(body)) }),
    );
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::BudgetCheck)));
    verify_module(&module).expect("loop budget");
}

#[test]
fn goal_provider_call_publishes_with_effect_pair() {
    let mut session = Session::new();
    let make_request = || {
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
            a: Integer::from_i64(12),
            b: Integer::from_i64(8),
        })))
    };
    let module = compile(&mut session, make_request());
    assert_eq!(module.provider_calls.len(), 1);
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::CallProvider)));
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::PublishResult)));
    assert!(module.regions[0].blocks.iter().any(|b| {
        b.operations.iter().any(|op| matches!(op.kind, OperationKind::CallProvider { .. }) && op.effect_in.is_some())
    }));
    verify_module(&module).expect("provider effects");

    let result_id = athena_engine::execution::execute_ir_request(&mut session, make_request()).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    let provenance = loaded.provenance.as_ref().expect("provenance");
    assert_eq!(provenance.request_kind, "CallProvider");
    assert!(provenance.capability_fingerprint.is_some());
}

#[test]
fn tampered_fingerprint_fails_reverify() {
    let mut session = Session::new();
    let term = session.builder().int(9, Default::default());
    let mut module = compile(&mut session, AthenaRequest::Term(term));
    module.fingerprint = ModuleFingerprint(0);
    let err = verify_module(&module).expect_err("tampered");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("fingerprint_mismatch"));
}
