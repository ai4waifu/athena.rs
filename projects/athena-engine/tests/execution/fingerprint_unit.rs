//! `ModuleFingerprint` 内容合同：常量 / 终结器 / 操作数必须进入指纹。

use athena_engine::execution::ir::{
    ConstantId, ConstantValue, ExecutionModule, ExecutionValueType, ModuleFingerprint, Operation, OperationKind, SsaValueId, Terminator,
    verify_module,
};

fn boolean_module(value: bool) -> ExecutionModule {
    let mut module = ExecutionModule::empty();
    module.constants.push(ConstantValue::Boolean(value));
    module.regions[0].result_types = vec![ExecutionValueType::Boolean];
    module.regions[0].blocks[0].operations.push(Operation {
        result: Some(SsaValueId(0)),
        result_type: ExecutionValueType::Boolean,
        kind: OperationKind::Constant { constant: ConstantId(0) },
        effect_in: None,
        effect_out: None,
    });
    module.regions[0].blocks[0].terminator = Terminator::return_value(SsaValueId(0));
    module.fingerprint = ModuleFingerprint::of_module(&module);
    module
}

#[test]
fn constant_payload_changes_fingerprint() {
    let t = boolean_module(true);
    let f = boolean_module(false);
    assert_ne!(t.fingerprint, f.fingerprint);
    assert!(verify_module(&t).is_ok());
    assert!(verify_module(&f).is_ok());
}

#[test]
fn stale_fingerprint_after_constant_edit_fails_verify() {
    let original = boolean_module(true);
    let mut changed = original.clone();
    changed.constants[0] = ConstantValue::Boolean(false);
    // 故意保留旧指纹：内容变了必须 mismatch。
    assert_ne!(original.fingerprint, ModuleFingerprint::of_module(&changed));
    assert!(verify_module(&changed).is_err());
}

#[test]
fn terminator_kind_changes_fingerprint() {
    let mut ret = boolean_module(true);
    let mut reject = ret.clone();
    reject.regions[0].blocks[0].terminator = Terminator::Reject { exit: None };
    reject.fingerprint = ModuleFingerprint::of_module(&reject);
    assert_ne!(ret.fingerprint, reject.fingerprint);
    assert!(verify_module(&reject).is_ok());
}
