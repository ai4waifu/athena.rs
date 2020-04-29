//! 自 `src/execution/vm.rs` 迁出的 VM bridge 测试。

use athena_engine::{
    Session,
    execution::vm::{empty_vm_module, execute_vm_module, vm_config_from_session},
};
use athena_vm::{CancellationToken, Instruction, Interpreter, SlotValue, VmConstant, VmExit, VmExecutor, VmModule};

#[test]
fn session_projects_vm_config_and_runs_empty_module() {
    let session = Session::new();
    let module = empty_vm_module();
    let exit = execute_vm_module(&session, &module).expect("vm execute");
    assert_eq!(exit, VmExit::Returned);
    assert_ne!(module.fingerprint.0, 0);
}

#[test]
fn safepoint_then_return_under_budget() {
    let session = Session::new();
    let module = VmModule::from_instructions(vec![Instruction::Safepoint, Instruction::Return], 0);
    let mut cfg = vm_config_from_session(&session);
    cfg.max_steps = Some(8);
    let mut interpreter = Interpreter::new();
    let exit = interpreter.execute(&module, &cfg).expect("vm execute");
    assert_eq!(exit, VmExit::Returned);
}

#[test]
fn cancel_token_projects_to_cancelled_exit() {
    let session = Session::new();
    let token = CancellationToken::new();
    token.cancel();
    let module = VmModule::from_instructions(vec![Instruction::Safepoint, Instruction::Return], 1);
    let cfg = vm_config_from_session(&session).with_cancellation(token);
    let mut interpreter = Interpreter::new();
    let exit = interpreter.execute(&module, &cfg).expect("vm execute");
    assert_eq!(exit, VmExit::Cancelled);
}

#[test]
fn bridge_runs_load_constant_guard_true() {
    let session = Session::new();
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::Guard { predicate: 0 },
            Instruction::Return,
        ],
        vec![VmConstant::Boolean(true)],
        1,
    );
    let mut interpreter = Interpreter::new();
    let cfg = vm_config_from_session(&session);
    let exit = interpreter.execute(&module, &cfg).expect("vm execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(interpreter.slots().get(0), Some(SlotValue::Boolean(true)));
}

#[test]
fn bridge_maps_reject_exit() {
    let session = Session::new();
    let module = VmModule::from_instructions(vec![Instruction::Reject], 0);
    let exit = execute_vm_module(&session, &module).expect("vm execute");
    assert_eq!(exit, VmExit::Rejected);
}

#[test]
fn execution_host_not_via_vm_interpreter() {
    use athena_engine::execution::vm::ExecutionHost;
    use athena_ir::SemanticOperator;
    use athena_vm::SemanticOpId;

    let session = Session::new();
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::apply_semantic1(1, SemanticOpId(SemanticOperator::Not.discriminant()), 0),
            Instruction::Return,
        ],
        vec![VmConstant::Boolean(true)],
        2,
    );
    let mut interpreter = Interpreter::new();
    let mut host = ExecutionHost::new();
    let cfg = vm_config_from_session(&session);
    let exit = interpreter.execute_with_host(&module, &cfg, &mut host).expect("vm execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(interpreter.slots().get(1), Some(SlotValue::Boolean(false)));
}

#[test]
fn execution_host_and_or_via_vm_interpreter() {
    use athena_engine::execution::vm::ExecutionHost;
    use athena_ir::SemanticOperator;
    use athena_vm::SemanticOpId;

    let session = Session::new();
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::LoadConstant { dst: 1, constant: 1 },
            Instruction::apply_semantic2(2, SemanticOpId(SemanticOperator::And.discriminant()), 0, 1),
            Instruction::apply_semantic2(3, SemanticOpId(SemanticOperator::Or.discriminant()), 0, 1),
            Instruction::Return,
        ],
        vec![VmConstant::Boolean(true), VmConstant::Boolean(false)],
        4,
    );
    let mut interpreter = Interpreter::new();
    let mut host = ExecutionHost::new();
    let cfg = vm_config_from_session(&session);
    let exit = interpreter.execute_with_host(&module, &cfg, &mut host).expect("vm execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(interpreter.slots().get(2), Some(SlotValue::Boolean(false)));
    assert_eq!(interpreter.slots().get(3), Some(SlotValue::Boolean(true)));
}

#[test]
fn execution_host_trueq_equal_unequal() {
    use athena_engine::execution::vm::ExecutionHost;
    use athena_ir::SemanticOperator;
    use athena_vm::SemanticOpId;

    let session = Session::new();
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::LoadConstant { dst: 1, constant: 1 },
            Instruction::apply_semantic1(2, SemanticOpId(SemanticOperator::TrueQ.discriminant()), 0),
            Instruction::apply_semantic2(3, SemanticOpId(SemanticOperator::Equal.discriminant()), 0, 1),
            Instruction::apply_semantic2(4, SemanticOpId(SemanticOperator::Unequal.discriminant()), 0, 1),
            Instruction::apply_semantic2(5, SemanticOpId(SemanticOperator::Identical.discriminant()), 0, 0),
            Instruction::Return,
        ],
        vec![VmConstant::Boolean(true), VmConstant::Boolean(false)],
        6,
    );
    let mut interpreter = Interpreter::new();
    let mut host = ExecutionHost::new();
    let cfg = vm_config_from_session(&session);
    let exit = interpreter.execute_with_host(&module, &cfg, &mut host).expect("vm execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(interpreter.slots().get(2), Some(SlotValue::Boolean(true)));
    assert_eq!(interpreter.slots().get(3), Some(SlotValue::Boolean(false)));
    assert_eq!(interpreter.slots().get(4), Some(SlotValue::Boolean(true)));
    assert_eq!(interpreter.slots().get(5), Some(SlotValue::Boolean(true)));
}
