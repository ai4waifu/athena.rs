//! 自 `src/execution/ir/mod.rs` 迁出的原内联测试。

use athena_engine::{Session, execution::ir::*};

#[test]
fn empty_module_has_stable_shape() {
    let module = ExecutionModule::empty();
    assert_eq!(module.regions.len(), 1);
    assert_eq!(module.entry_region(), Some(RegionId(0)));
    assert_eq!(module.fingerprint, ModuleFingerprint::of_module(&module));
}
