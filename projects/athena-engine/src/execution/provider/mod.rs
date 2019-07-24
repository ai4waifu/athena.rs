//! Typed provider call surface for `ExecutionIR` (capability + verifier handoff).
//!
//! Provider-private kernel artifacts are not a second Athena IR. They bind to a
//! [`ProviderCallDescriptor`](crate::execution::ir::ProviderCallDescriptor) only.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use athena_types::OperatorId;

use crate::execution::ir::{ExecutionValueType, ProviderCallDescriptor, ProviderCallId};

/// Capability snapshot required by a provider call site.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderCapabilitySnapshot {
    /// Opaque capability fingerprint (backend-defined bits).
    pub fingerprint: u64,
}

impl ProviderCapabilitySnapshot {
    /// Derive a session-local capability fingerprint from a closed operator id.
    pub fn from_operator(operator: OperatorId) -> Self {
        let mut hasher = DefaultHasher::new();
        0x5052_4f56_4341_5045u64.hash(&mut hasher); // "PROVCAPE"
        operator.0.hash(&mut hasher);
        Self { fingerprint: hasher.finish() }
    }
}

/// Handoff from executor to provider verifier / admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCallHandoff {
    /// Descriptor from the module table.
    pub descriptor: ProviderCallDescriptor,
    /// Required capabilities.
    pub capabilities: ProviderCapabilitySnapshot,
}

impl ProviderCallHandoff {
    /// Build a handoff from operator identity.
    pub fn from_operator(id: ProviderCallId, operator: OperatorId) -> Self {
        Self {
            descriptor: ProviderCallDescriptor::new(id, operator, ExecutionValueType::Unknown),
            capabilities: ProviderCapabilitySnapshot::from_operator(operator),
        }
    }

    /// Build a handoff from a compiled descriptor table entry.
    pub fn from_descriptor(descriptor: ProviderCallDescriptor) -> Self {
        let capabilities = ProviderCapabilitySnapshot::from_operator(descriptor.operator);
        Self { descriptor, capabilities }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::ir::ProviderCallId;

    #[test]
    fn capability_fingerprint_is_stable_for_operator() {
        let a = ProviderCapabilitySnapshot::from_operator(OperatorId(7));
        let b = ProviderCapabilitySnapshot::from_operator(OperatorId(7));
        let c = ProviderCapabilitySnapshot::from_operator(OperatorId(8));
        assert_eq!(a, b);
        assert_ne!(a.fingerprint, c.fingerprint);
        assert_ne!(a.fingerprint, 0);
    }

    #[test]
    fn handoff_from_descriptor_binds_operator_capability() {
        let descriptor = ProviderCallDescriptor::new(ProviderCallId(0), OperatorId(3), ExecutionValueType::Unit);
        let handoff = ProviderCallHandoff::from_descriptor(descriptor.clone());
        assert_eq!(handoff.descriptor, descriptor);
        assert_eq!(handoff.capabilities, ProviderCapabilitySnapshot::from_operator(OperatorId(3)));
    }
}
