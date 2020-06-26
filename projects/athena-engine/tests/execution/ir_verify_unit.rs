//! 自 `src/execution/ir/verify.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    execution::ir::{
        BasicBlock, BlockEdge, BlockId, ConstantId, ConstantValue, ExecutionModule, ExecutionValueType, Operation, Region, RegionId,
        SsaValueId, Terminator, *,
    },
};
use athena_types::{Diagnostic, DiagnosticCode, Result};
use std::collections::{HashMap, HashSet};

#[test]
fn empty_module_verifies() {
    let module = ExecutionModule::empty();
    verify_module(&module).expect("empty module");
}

#[test]
fn constant_return_verifies() {
    let v0 = SsaValueId(0);
    let block = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(v0),
            result_type: ExecutionValueType::Boolean,
            kind: athena_engine::execution::ir::OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(v0),
    };
    let region = Region::from_entry_block(RegionId(0), block, vec![ExecutionValueType::Boolean]);
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
    verify_module(&module).expect("constant return");
}

#[test]
fn use_before_def_rejected() {
    let block =
        BasicBlock { id: BlockId(0), parameters: Vec::new(), operations: Vec::new(), terminator: Terminator::return_value(SsaValueId(99)) };
    let region = Region::from_entry_block(RegionId(0), block, Vec::new());
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: Vec::new(),
        captured_roots: Vec::new(),
        regions: vec![region],
        effect_edges: Vec::new(),
        exits: Vec::new(),
        provider_calls: Vec::new(),
        fingerprint: ModuleFingerprint(0),
    };
    module.fingerprint = ModuleFingerprint::of_module(&module);
    assert!(verify_module(&module).is_err());
}

#[test]
fn branch_targets_must_exist() {
    let cond = SsaValueId(0);
    let entry = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(cond),
            result_type: ExecutionValueType::Boolean,
            kind: athena_engine::execution::ir::OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::Branch { condition: cond, then_edge: BlockEdge::jump(BlockId(1)), else_edge: BlockEdge::jump(BlockId(2)) },
    };
    let region = Region { id: RegionId(0), entry: BlockId(0), blocks: vec![entry], result_types: Vec::new() };
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
    assert!(verify_module(&module).is_err());
}

#[test]
fn fingerprint_mismatch_rejected() {
    let mut module = ExecutionModule::empty();
    module.fingerprint = ModuleFingerprint(0xdead_beef);
    let err = verify_module(&module).expect_err("tampered fingerprint");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("fingerprint_mismatch"));
}

#[test]
fn effect_token_pair_mismatch_rejected() {
    let v0 = SsaValueId(0);
    let block = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(v0),
            result_type: ExecutionValueType::Unit,
            kind: athena_engine::execution::ir::OperationKind::Constant { constant: ConstantId(0) },
            effect_in: Some(athena_engine::execution::ir::EffectToken(0)),
            effect_out: None,
        }],
        terminator: Terminator::return_value(v0),
    };
    let region = Region::from_entry_block(RegionId(0), block, vec![ExecutionValueType::Unit]);
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: vec![ConstantValue::Unit],
        captured_roots: Vec::new(),
        regions: vec![region],
        effect_edges: Vec::new(),
        exits: Vec::new(),
        provider_calls: Vec::new(),
        fingerprint: ModuleFingerprint(0),
    };
    module.fingerprint = ModuleFingerprint::of_module(&module);
    let err = verify_module(&module).expect_err("unpaired effect");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("effect_token_pair_mismatch"));
}

#[test]
fn effect_predecessor_must_exist() {
    use athena_engine::execution::ir::{EffectEdge, EffectKind, EffectToken};

    let mut module = ExecutionModule::empty();
    module.effect_edges.push(EffectEdge::after(EffectToken(0), EffectToken(99), EffectKind::WriteBinding));
    module.fingerprint = ModuleFingerprint::of_module(&module);
    let err = verify_module(&module).expect_err("unknown predecessor");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("effect_predecessor_unknown"));
}

#[test]
fn guard_exit_must_be_declared() {
    use athena_engine::execution::ir::{GuardFailure, OperationKind};

    let pred = SsaValueId(0);
    let block = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![
            Operation {
                result: Some(pred),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: ConstantId(0) },
                effect_in: None,
                effect_out: None,
            },
            Operation {
                result: None,
                result_type: ExecutionValueType::Unit,
                kind: OperationKind::Guard { predicate: pred, on_failure: GuardFailure::Exit(athena_engine::execution::ir::ExitId(7)) },
                effect_in: None,
                effect_out: None,
            },
        ],
        terminator: Terminator::return_value(pred),
    };
    let region = Region::from_entry_block(RegionId(0), block, vec![ExecutionValueType::Boolean]);
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
    let err = verify_module(&module).expect_err("missing exit");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("guard_exit_unknown"));
}

#[test]
fn non_dominating_cross_block_use_rejected() {
    // entry → then / else → join
    // then defines %1, else uses %1 without block param → dominance fail
    let cond = SsaValueId(0);
    let then_val = SsaValueId(1);
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
        terminator: Terminator::Branch { condition: cond, then_edge: BlockEdge::jump(BlockId(1)), else_edge: BlockEdge::jump(BlockId(2)) },
    };
    let then_block = BasicBlock {
        id: BlockId(1),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(then_val),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(then_val),
    };
    let else_block =
        BasicBlock { id: BlockId(2), parameters: Vec::new(), operations: Vec::new(), terminator: Terminator::return_value(then_val) };
    let region = Region {
        id: RegionId(0),
        entry: BlockId(0),
        blocks: vec![entry, then_block, else_block],
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
    let err = verify_module(&module).expect_err("non-dominating use");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("ssa_def_does_not_dominate_use"));
}

#[test]
fn cold_unreachable_handler_block_tolerated() {
    // entry → body → return；handler 无入边（Recover 冷路径形态）
    let v0 = SsaValueId(0);
    let v1 = SsaValueId(1);
    let entry = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(v0),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(v0),
    };
    let cold = BasicBlock {
        id: BlockId(1),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(v1),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(v1),
    };
    let region = Region { id: RegionId(0), entry: BlockId(0), blocks: vec![entry, cold], result_types: vec![ExecutionValueType::Boolean] };
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
    verify_module(&module).expect("cold unreachable tolerated");
}
