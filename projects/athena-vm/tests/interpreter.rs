//! 解释器骨架合同。

use athena_vm::{
    CancellationToken, Instruction, Interpreter, SlotValue, VmConfig, VmConstant, VmExit, VmExecutor, VmModule,
};

#[test]
fn empty_return_module_exits_returned() {
    let module = VmModule::empty_return();
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(vm.frames().depth(), 1);
}

#[test]
fn max_steps_budget() {
    let module = VmModule::from_instructions(
        vec![Instruction::Safepoint, Instruction::Safepoint, Instruction::Return],
        0,
    );
    let mut vm = Interpreter::new();
    let cfg = VmConfig::new().with_max_steps(1);
    let exit = vm.execute(&module, &cfg).expect("execute");
    assert_eq!(exit, VmExit::BudgetExceeded);
}

#[test]
fn cancel_at_safepoint() {
    let token = CancellationToken::new();
    token.cancel();
    let module = VmModule::from_instructions(vec![Instruction::Safepoint, Instruction::Return], 2);
    let mut vm = Interpreter::new();
    let cfg = VmConfig::new().with_cancellation(token);
    let exit = vm.execute(&module, &cfg).expect("execute");
    assert_eq!(exit, VmExit::Cancelled);
    assert_eq!(vm.slots().len(), 2);
}

#[test]
fn empty_instructions_diagnostic() {
    let module = VmModule::from_instructions(Vec::new(), 0);
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert!(matches!(exit, VmExit::Diagnostic(_)));
}

#[test]
fn load_constant_move_and_return() {
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::Move { dst: 1, src: 0 },
            Instruction::Return,
        ],
        vec![VmConstant::Boolean(true)],
        2,
    );
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(vm.slots().get(0), Some(SlotValue::Boolean(true)));
    assert_eq!(vm.slots().get(1), Some(SlotValue::Boolean(true)));
}

#[test]
fn guard_false_rejects() {
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::Guard { predicate: 0 },
            Instruction::Return,
        ],
        vec![VmConstant::Boolean(false)],
        1,
    );
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert_eq!(exit, VmExit::Rejected);
}

#[test]
fn explicit_reject() {
    let module = VmModule::from_instructions(vec![Instruction::Reject], 0);
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert_eq!(exit, VmExit::Rejected);
}
