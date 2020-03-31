//! 解释器骨架合同。

use athena_vm::{CancellationToken, Instruction, Interpreter, VmConfig, VmExit, VmExecutor, VmModule};

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
