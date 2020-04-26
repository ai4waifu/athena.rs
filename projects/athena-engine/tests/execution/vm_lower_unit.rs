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

#[test]
fn lower_rejects_branch_with_edge_args() {
    let cond = SsaValueId(0);
    let param = SsaValueId(1);
    let entry = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(cond),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::Branch {
            condition: cond,
            then_edge: BlockEdge {
                target: BlockId(1),
                arguments: vec![cond],
            },
            else_edge: BlockEdge {
                target: BlockId(1),
                arguments: vec![cond],
            },
        },
    };
    let ret = BasicBlock {
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
        blocks: vec![entry, ret],
        result_types: vec![ExecutionValueType::Boolean],
    };
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
    let err = try_lower_linear_boolean_module(&module).expect_err("edge args");
    let reason = err.details.get("reason").map(|v| v.to_string());
    assert!(
        reason.as_deref() == Some("lower_rejects_branch_args")
            || reason.as_deref() == Some("lower_rejects_block_parameters"),
        "unexpected reason {reason:?}"
    );
}
