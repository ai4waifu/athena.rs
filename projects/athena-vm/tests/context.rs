//! `VmExecutionContext` / safepoint 合同。

use std::rc::Rc;

use athena_gc::{GcHeap, GcMode, HeapBudget};
use athena_vm::{
    ExecutionLease, Instruction, Interpreter, NullHost, VmConfig, VmExecutionContext, VmExecutor, VmExit, VmModule,
};

#[test]
fn safepoint_instruction_counts_with_lease() {
    let heap = GcHeap::new(HeapBudget::default());
    let mut lease = ExecutionLease::new(Rc::clone(&heap));
    let mut ctx = VmExecutionContext::with_lease(&mut lease);
    assert!(ctx.has_lease());

    let module = VmModule::from_instructions(
        vec![Instruction::Safepoint, Instruction::Safepoint, Instruction::Return],
        0,
    );
    let mut vm = Interpreter::new();
    let cfg = VmConfig::new().with_gc_mode(GcMode::Deferred);
    let exit = vm
        .execute_with_context(&module, &cfg, &mut NullHost, &mut ctx)
        .expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(ctx.safepoint_count, 2);
    assert_eq!(ctx.collect_count, 0);
}

#[test]
fn auto_safepoint_collects_when_pressure_hit() {
    let heap = GcHeap::new(HeapBudget::default());
    {
        let h = heap.borrow();
        h.gc().record_allocation(h.gc().auto_threshold_bytes().saturating_add(1));
        assert!(h.gc().should_collect_after_alloc());
    }
    let mut lease = ExecutionLease::new(Rc::clone(&heap));
    let mut ctx = VmExecutionContext::with_lease(&mut lease);
    let module = VmModule::from_instructions(vec![Instruction::Safepoint, Instruction::Return], 0);
    let mut vm = Interpreter::new();
    let cfg = VmConfig::new().with_gc_mode(GcMode::Auto);
    let before = heap.borrow().stats().collect_count;
    let exit = vm
        .execute_with_context(&module, &cfg, &mut NullHost, &mut ctx)
        .expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(ctx.safepoint_count, 1);
    assert_eq!(ctx.collect_count, 1);
    assert!(heap.borrow().stats().collect_count > before);
}

#[test]
fn detached_context_still_counts_safepoints() {
    let mut ctx = VmExecutionContext::detached();
    let module = VmModule::from_instructions(vec![Instruction::Safepoint, Instruction::Return], 0);
    let mut vm = Interpreter::new();
    let exit = vm
        .execute_with_context(&module, &VmConfig::new(), &mut NullHost, &mut ctx)
        .expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert!(!ctx.has_lease());
    assert_eq!(ctx.safepoint_count, 1);
    assert_eq!(ctx.collect_count, 0);
}
