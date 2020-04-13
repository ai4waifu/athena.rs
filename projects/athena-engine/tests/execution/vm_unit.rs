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
