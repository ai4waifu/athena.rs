//! ReferenceExecutor ↔ athena-vm 行为 parity（取消 / 预算 / 拒绝 / 布尔值）。

use athena_engine::{
    Session,
    execution::{
        ir::{
            BasicBlock, BlockId, ConstantId, ConstantValue, ExecutionModule, ExecutionValueType, GuardFailure, ModuleFingerprint, Operation,
            OperationKind, Region, RegionId, SsaValueId, Terminator, verify_module,
        },
        reference::ReferenceExecutor,
        vm::{ExecutionHost, try_lower_verified_cfg_module},
    },
};
use athena_ir::SemanticOperator;
use athena_vm::{CancellationToken, Interpreter, SlotValue, VmConfig, VmExecutor, VmExit};

fn not_module(input: bool) -> ExecutionModule {
    let load = SsaValueId(0);
    let out = SsaValueId(1);
    let block = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![
            Operation {
                result: Some(load),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(0) },
                effect_in: None,
                effect_out: None,
            },
            Operation {
                result: Some(out),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::ApplySemanticOperator { operator: SemanticOperator::Not, args: vec![load] },
                effect_in: None,
                effect_out: None,
            },
        ],
        terminator: Terminator::return_value(out),
    };
    let region = Region { id: RegionId(0), entry: BlockId(0), blocks: vec![block], result_types: vec![ExecutionValueType::Boolean] };
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: vec![ConstantValue::boolean(input)],
        captured_roots: Vec::new(),
        regions: vec![region],
        effect_edges: Vec::new(),
        exits: Vec::new(),
        provider_calls: Vec::new(),
        fingerprint: ModuleFingerprint(0),
    };
    module.fingerprint = ModuleFingerprint::of_module(&module);
    module
}

/// Entry rejects；孤立 return 块仅满足 VM lowerer 的「至少一处 Return」合同。
fn reject_module() -> ExecutionModule {
    let entry = BasicBlock { id: BlockId(0), parameters: Vec::new(), operations: Vec::new(), terminator: Terminator::Reject { exit: None } };
    let ret_v = SsaValueId(0);
    let ret = BasicBlock {
        id: BlockId(1),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(ret_v),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(ret_v),
    };
    let region = Region { id: RegionId(0), entry: BlockId(0), blocks: vec![entry, ret], result_types: vec![ExecutionValueType::Boolean] };
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: vec![ConstantValue::boolean(true)],
        captured_roots: Vec::new(),
        regions: vec![region],
        effect_edges: Vec::new(),
        exits: Vec::new(),
        provider_calls: Vec::new(),
        fingerprint: ModuleFingerprint(0),
    };
    module.fingerprint = ModuleFingerprint::of_module(&module);
    module
}

fn run_vm(session: &mut Session, module: &ExecutionModule, config: &VmConfig) -> athena_types::Result<VmExit> {
    let lowered = try_lower_verified_cfg_module(module)?;
    let mut interpreter = Interpreter::new();
    let mut host = ExecutionHost::new(session, Vec::new(), None, Vec::new());
    interpreter.execute_with_host(&lowered.module, config, &mut host)
}

fn reason_of(err: &athena_types::Diagnostic) -> Option<String> {
    err.details.get("reason").map(|v| v.to_string())
}

#[test]
fn parity_not_boolean_value() {
    let mut session = Session::new();
    let module = not_module(true);
    verify_module(&module).expect("verify");

    let ref_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("reference");
    let ref_term = session.results.get(ref_id).and_then(|r| r.symbolic_term).expect("term");
    match session.arena.get(ref_term) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(false))) => {}
        other => panic!("reference expected false, got {other:?}"),
    }

    let cfg = VmConfig::default();
    let lowered = try_lower_verified_cfg_module(&module).expect("lower");
    let mut interpreter = Interpreter::new();
    let mut host = ExecutionHost::new(&mut session, Vec::new(), None, Vec::new());
    let exit = interpreter.execute_with_host(&lowered.module, &cfg, &mut host).expect("vm");
    assert_eq!(exit, VmExit::Returned);
    let slot = interpreter.last_return_slot().unwrap_or(lowered.result_slot);
    assert_eq!(interpreter.slots().get(slot), Some(SlotValue::Boolean(false)));
}

#[test]
fn parity_cancelled() {
    let mut session = Session::new();
    let module = not_module(true);
    let token = CancellationToken::new();
    token.cancel();
    let config = VmConfig::default().with_cancellation(token);

    let ref_err = ReferenceExecutor::new().execute_configured(&mut session, &module, None, &config).expect_err("reference cancel");
    assert_eq!(reason_of(&ref_err).as_deref(), Some("cancelled"));

    let vm_exit = run_vm(&mut session, &module, &config).expect("vm runs to exit");
    assert_eq!(vm_exit, VmExit::Cancelled);
}

#[test]
fn parity_budget_exceeded() {
    let mut session = Session::new();
    let module = not_module(true);
    let config = VmConfig::default().with_max_steps(0);

    let ref_err = ReferenceExecutor::new().execute_configured(&mut session, &module, None, &config).expect_err("reference budget");
    assert_eq!(reason_of(&ref_err).as_deref(), Some("budget_exceeded"));

    let vm_exit = run_vm(&mut session, &module, &config).expect("vm runs to exit");
    assert_eq!(vm_exit, VmExit::BudgetExceeded);
}

#[test]
fn parity_reject_terminator() {
    let mut session = Session::new();
    let module = reject_module();
    verify_module(&module).expect("verify");

    let ref_err = ReferenceExecutor::new().execute(&mut session, &module, None).expect_err("reference reject");
    assert_eq!(reason_of(&ref_err).as_deref(), Some("rejected"));

    let vm_exit = run_vm(&mut session, &module, &VmConfig::default()).expect("vm");
    assert_eq!(vm_exit, VmExit::Rejected);
}

fn guard_reject_module(pred: bool) -> ExecutionModule {
    let p = SsaValueId(0);
    let out = SsaValueId(1);
    let block = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![
            Operation {
                result: Some(p),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(0) },
                effect_in: None,
                effect_out: None,
            },
            Operation {
                result: None,
                result_type: ExecutionValueType::Unit,
                kind: OperationKind::Guard { predicate: p, on_failure: GuardFailure::Reject },
                effect_in: None,
                effect_out: None,
            },
            Operation {
                result: Some(out),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(1) },
                effect_in: None,
                effect_out: None,
            },
        ],
        terminator: Terminator::return_value(out),
    };
    let region = Region { id: RegionId(0), entry: BlockId(0), blocks: vec![block], result_types: vec![ExecutionValueType::Boolean] };
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: vec![ConstantValue::boolean(pred), ConstantValue::boolean(true)],
        captured_roots: Vec::new(),
        regions: vec![region],
        effect_edges: Vec::new(),
        exits: Vec::new(),
        provider_calls: Vec::new(),
        fingerprint: ModuleFingerprint(0),
    };
    module.fingerprint = ModuleFingerprint::of_module(&module);
    module
}

#[test]
fn parity_guard_reject() {
    let mut session = Session::new();
    let module = guard_reject_module(false);
    verify_module(&module).expect("verify");

    let ref_err = ReferenceExecutor::new().execute(&mut session, &module, None).expect_err("reference guard");
    assert_eq!(reason_of(&ref_err).as_deref(), Some("rejected"));

    let vm_exit = run_vm(&mut session, &module, &VmConfig::default()).expect("vm");
    assert_eq!(vm_exit, VmExit::Rejected);
}

#[test]
fn parity_guard_pass() {
    let mut session = Session::new();
    let module = guard_reject_module(true);
    verify_module(&module).expect("verify");

    let ref_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("reference");
    let ref_term = session.results.get(ref_id).and_then(|r| r.symbolic_term).expect("term");
    match session.arena.get(ref_term) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(true))) => {}
        other => panic!("reference expected true, got {other:?}"),
    }

    let lowered = try_lower_verified_cfg_module(&module).expect("lower");
    let mut interpreter = Interpreter::new();
    let mut host = ExecutionHost::new(&mut session, Vec::new(), None, Vec::new());
    let exit = interpreter.execute_with_host(&lowered.module, &VmConfig::default(), &mut host).expect("vm");
    assert_eq!(exit, VmExit::Returned);
    let slot = interpreter.last_return_slot().unwrap_or(lowered.result_slot);
    assert_eq!(interpreter.slots().get(slot), Some(SlotValue::Boolean(true)));
}
