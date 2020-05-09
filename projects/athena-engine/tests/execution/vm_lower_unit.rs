//! 线性 / 分支 Boolean IR → VM 降级测试。

use athena_engine::{
    Session,
    execution::{
        ir::{
            BasicBlock, BlockEdge, BlockId, ConstantId, ConstantValue, ExecutionModule, ExecutionValueType, ModuleFingerprint,
            Operation, OperationKind, Region, RegionId, SsaValueId, Terminator,
        },
        vm::{execute_linear_boolean_on_vm, try_lower_linear_boolean_module},
    },
};
use athena_ir::SemanticOperator;
use athena_vm::SlotValue;

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
                kind: OperationKind::ApplySemanticOperator {
                    operator: SemanticOperator::Not,
                    args: vec![load],
                },
                effect_in: None,
                effect_out: None,
            },
        ],
        terminator: Terminator::return_value(out),
    };
    let region = Region {
        id: RegionId(0),
        entry: BlockId(0),
        blocks: vec![block],
        result_types: vec![ExecutionValueType::Boolean],
    };
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

fn branch_module(cond: bool) -> ExecutionModule {
    let c = SsaValueId(0);
    let then_v = SsaValueId(1);
    let else_v = SsaValueId(2);
    let entry = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(c),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::Branch {
            condition: c,
            then_edge: BlockEdge::jump(BlockId(1)),
            else_edge: BlockEdge::jump(BlockId(2)),
        },
    };
    let then_block = BasicBlock {
        id: BlockId(1),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(then_v),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(1) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(then_v),
    };
    let else_block = BasicBlock {
        id: BlockId(2),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(else_v),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(2) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(else_v),
    };
    let region = Region {
        id: RegionId(0),
        entry: BlockId(0),
        blocks: vec![entry, then_block, else_block],
        result_types: vec![ExecutionValueType::Boolean],
    };
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: vec![
            ConstantValue::boolean(cond),
            ConstantValue::boolean(true),
            ConstantValue::boolean(false),
        ],
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
fn lower_and_execute_not_on_vm() {
    let session = Session::new();
    let module = not_module(true);
    let lowered = try_lower_linear_boolean_module(&module).expect("lower");
    assert_eq!(lowered.result_slot, 1);
    let value = execute_linear_boolean_on_vm(&session, &module).expect("vm");
    assert_eq!(value, SlotValue::Boolean(false));
}

#[test]
fn lower_and_execute_boolean_branch_on_vm() {
    let session = Session::new();
    let then_mod = branch_module(true);
    assert_eq!(
        execute_linear_boolean_on_vm(&session, &then_mod).expect("then"),
        SlotValue::Boolean(true)
    );
    let else_mod = branch_module(false);
    assert_eq!(
        execute_linear_boolean_on_vm(&session, &else_mod).expect("else"),
        SlotValue::Boolean(false)
    );
}

fn edge_arg_phi_module(cond: bool, then_val: bool, else_val: bool) -> ExecutionModule {
    let c = SsaValueId(0);
    let then_v = SsaValueId(1);
    let else_v = SsaValueId(2);
    let param = SsaValueId(3);
    let entry = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![
            Operation {
                result: Some(c),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(0) },
                effect_in: None,
                effect_out: None,
            },
            Operation {
                result: Some(then_v),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(1) },
                effect_in: None,
                effect_out: None,
            },
            Operation {
                result: Some(else_v),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(2) },
                effect_in: None,
                effect_out: None,
            },
        ],
        terminator: Terminator::Branch {
            condition: c,
            then_edge: BlockEdge {
                target: BlockId(1),
                arguments: vec![then_v],
            },
            else_edge: BlockEdge {
                target: BlockId(1),
                arguments: vec![else_v],
            },
        },
    };
    let join = BasicBlock {
        id: BlockId(1),
        parameters: vec![athena_engine::execution::ir::BlockParameter {
            value: param,
            ty: ExecutionValueType::Boolean,
        }],
        operations: Vec::new(),
        terminator: Terminator::return_value(param),
    };
    let region = Region {
        id: RegionId(0),
        entry: BlockId(0),
        blocks: vec![entry, join],
        result_types: vec![ExecutionValueType::Boolean],
    };
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: vec![
            ConstantValue::boolean(cond),
            ConstantValue::boolean(then_val),
            ConstantValue::boolean(else_val),
        ],
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
fn lower_and_execute_boolean_edge_arg_phi_on_vm() {
    let session = Session::new();
    let then_mod = edge_arg_phi_module(true, true, false);
    assert!(try_lower_linear_boolean_module(&then_mod).is_ok());
    assert_eq!(
        execute_linear_boolean_on_vm(&session, &then_mod).expect("then"),
        SlotValue::Boolean(true)
    );
    let else_mod = edge_arg_phi_module(false, true, false);
    assert_eq!(
        execute_linear_boolean_on_vm(&session, &else_mod).expect("else"),
        SlotValue::Boolean(false)
    );
}

fn guarded_boolean_module(pred: bool) -> ExecutionModule {
    use athena_engine::execution::ir::GuardFailure;
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
                kind: OperationKind::Guard {
                    predicate: p,
                    on_failure: GuardFailure::Reject,
                },
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
    let region = Region {
        id: RegionId(0),
        entry: BlockId(0),
        blocks: vec![block],
        result_types: vec![ExecutionValueType::Boolean],
    };
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
fn lower_guard_reject_passes_on_true() {
    let session = Session::new();
    let module = guarded_boolean_module(true);
    assert_eq!(
        execute_linear_boolean_on_vm(&session, &module).expect("pass"),
        SlotValue::Boolean(true)
    );
}

#[test]
fn lower_guard_reject_fails_on_false() {
    let session = Session::new();
    let module = guarded_boolean_module(false);
    let err = execute_linear_boolean_on_vm(&session, &module).expect_err("reject");
    assert_eq!(
        err.details.get("reason").map(|v| v.to_string()).as_deref(),
        Some("rejected")
    );
}

#[test]
fn lower_terminator_reject_on_else_edge() {
    let c = SsaValueId(0);
    let then_v = SsaValueId(1);
    let entry = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(c),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::Branch {
            condition: c,
            then_edge: BlockEdge::jump(BlockId(1)),
            else_edge: BlockEdge::jump(BlockId(2)),
        },
    };
    let then_block = BasicBlock {
        id: BlockId(1),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(then_v),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(1) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(then_v),
    };
    let else_block = BasicBlock {
        id: BlockId(2),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Reject { exit: None },
    };
    let region = Region {
        id: RegionId(0),
        entry: BlockId(0),
        blocks: vec![entry, then_block, else_block],
        result_types: vec![ExecutionValueType::Boolean],
    };
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: vec![ConstantValue::boolean(false), ConstantValue::boolean(true)],
        captured_roots: Vec::new(),
        regions: vec![region],
        effect_edges: Vec::new(),
        exits: Vec::new(),
        provider_calls: Vec::new(),
        fingerprint: ModuleFingerprint(0),
    };
    module.fingerprint = ModuleFingerprint::of_module(&module);
    let session = Session::new();
    assert!(try_lower_linear_boolean_module(&module).is_ok());
    let err = execute_linear_boolean_on_vm(&session, &module).expect_err("else reject");
    assert_eq!(
        err.details.get("reason").map(|v| v.to_string()).as_deref(),
        Some("rejected")
    );
}

fn interfering_swap_phi_module() -> ExecutionModule {
    let v_false = SsaValueId(0);
    let v_true = SsaValueId(1);
    let p0 = SsaValueId(2);
    let p1 = SsaValueId(3);
    let ret = SsaValueId(4);
    let entry = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![
            Operation {
                result: Some(v_false),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(0) },
                effect_in: None,
                effect_out: None,
            },
            Operation {
                result: Some(v_true),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(1) },
                effect_in: None,
                effect_out: None,
            },
        ],
        terminator: Terminator::Branch {
            condition: v_true,
            then_edge: BlockEdge {
                target: BlockId(1),
                arguments: vec![v_false, v_true],
            },
            else_edge: BlockEdge {
                target: BlockId(1),
                arguments: vec![v_false, v_true],
            },
        },
    };
    let loop_block = BasicBlock {
        id: BlockId(1),
        parameters: vec![
            athena_engine::execution::ir::BlockParameter {
                value: p0,
                ty: ExecutionValueType::Boolean,
            },
            athena_engine::execution::ir::BlockParameter {
                value: p1,
                ty: ExecutionValueType::Boolean,
            },
        ],
        operations: Vec::new(),
        terminator: Terminator::Branch {
            condition: p0,
            then_edge: BlockEdge {
                target: BlockId(2),
                arguments: vec![p1],
            },
            else_edge: BlockEdge {
                target: BlockId(1),
                arguments: vec![p1, p0],
            },
        },
    };
    let exit = BasicBlock {
        id: BlockId(2),
        parameters: vec![athena_engine::execution::ir::BlockParameter {
            value: ret,
            ty: ExecutionValueType::Boolean,
        }],
        operations: Vec::new(),
        terminator: Terminator::return_value(ret),
    };
    let region = Region {
        id: RegionId(0),
        entry: BlockId(0),
        blocks: vec![entry, loop_block, exit],
        result_types: vec![ExecutionValueType::Boolean],
    };
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: vec![ConstantValue::boolean(false), ConstantValue::boolean(true)],
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
fn lower_interfering_edge_arg_swap_uses_temps() {
    let session = Session::new();
    let module = interfering_swap_phi_module();
    let lowered = try_lower_linear_boolean_module(&module).expect("lower");
    let has_temp_move = lowered.module.instructions.iter().any(|insn| {
        matches!(insn, athena_vm::Instruction::Move { dst, .. } if *dst >= 5)
    });
    assert!(has_temp_move, "expected temporary Move slots for interfering phi");
    assert_eq!(
        execute_linear_boolean_on_vm(&session, &module).expect("swap"),
        SlotValue::Boolean(false)
    );
}

#[test]
fn lower_and_execute_load_term_atom_on_vm() {
    use athena_engine::execution::ir::{CapturedRoot, CapturedRootId};
    use athena_types::TermRef;

    let mut session = Session::new();
    let term = session.builder().int(11, Default::default());
    let load = SsaValueId(0);
    let block = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(load),
            result_type: ExecutionValueType::Term,
            kind: OperationKind::LoadTerm {
                root: CapturedRootId(0),
            },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(load),
    };
    let region = Region::from_entry_block(RegionId(0), block, vec![ExecutionValueType::Term]);
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: Vec::new(),
        captured_roots: vec![CapturedRoot::term(TermRef::new(term, session.arena.epoch()))],
        regions: vec![region],
        effect_edges: Vec::new(),
        exits: Vec::new(),
        provider_calls: Vec::new(),
        fingerprint: ModuleFingerprint(0),
    };
    module.fingerprint = ModuleFingerprint::of_module(&module);

    assert!(try_lower_linear_boolean_module(&module).is_ok());
    assert_eq!(
        execute_linear_boolean_on_vm(&session, &module).expect("vm"),
        SlotValue::Term(term)
    );
}
