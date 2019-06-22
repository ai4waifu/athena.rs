//! Typed provider call surface for `ExecutionIR` (capability + verifier handoff).
//!
//! Provider-private kernel artifacts are not a second Athena IR. They bind to a
//! [`ProviderCallDescriptor`](crate::execution::ir::ProviderCallDescriptor) only.

use athena_types::OperatorId;

use crate::execution::ir::{ExecutionValueType, ProviderCallDescriptor, ProviderCallId};

/// Capability snapshot required by a provider call site.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderCapabilitySnapshot {
    /// Opaque capability fingerprint (backend-defined bits).
    pub fingerprint: u64,
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
            capabilities: ProviderCapabilitySnapshot::default(),
        }
    }
}
