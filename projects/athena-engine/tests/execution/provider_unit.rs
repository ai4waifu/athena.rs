//! 自 `src/execution/provider/mod.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    execution::{
        ir::{ExecutionValueType, ProviderCallDescriptor, ProviderCallId},
        provider::*,
    },
};
use athena_types::ExtensionOperatorId;

#[test]
fn capability_fingerprint_is_stable_for_operator() {
    let a = ProviderCapabilitySnapshot::from_operator(ExtensionOperatorId(7));
    let b = ProviderCapabilitySnapshot::from_operator(ExtensionOperatorId(7));
    let c = ProviderCapabilitySnapshot::from_operator(ExtensionOperatorId(8));
    assert_eq!(a, b);
    assert_ne!(a.fingerprint, c.fingerprint);
    assert_ne!(a.fingerprint, 0);
}

#[test]
fn handoff_from_descriptor_binds_operator_capability() {
    let descriptor = ProviderCallDescriptor::new(ProviderCallId(0), ExtensionOperatorId(3), ExecutionValueType::Unit);
    let handoff = ProviderCallHandoff::from_descriptor(descriptor.clone());
    assert_eq!(handoff.descriptor, descriptor);
    assert_eq!(handoff.capabilities, ProviderCapabilitySnapshot::from_operator(ExtensionOperatorId(3)));
}
