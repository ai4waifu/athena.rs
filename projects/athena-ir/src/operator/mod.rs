//! Operator identities for IR applications.
//!
//! Core math / logic / structure ops use the closed [`SemanticOperator`] enum.
//! [`OperatorRegistry`] is an **extension display-name registry only** — not a
//! core semantic catalog. Dialect surface names are injected by product layers.

mod semantic;

use std::collections::HashMap;

use athena_types::OperatorId;

pub use semantic::{ApplicationHead, SemanticOperator};

/// Extension display-name ↔ [`OperatorId`] bidirectional table.
///
/// Not a core operator catalog. Core ops use [`SemanticOperator`].
#[derive(Debug, Clone, Default)]
pub struct OperatorRegistry {
    names: Vec<String>,
    by_name: HashMap<String, OperatorId>,
}

impl OperatorRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate or look up an extension operator id.
    pub fn intern(&mut self, name: &str) -> OperatorId {
        if let Some(id) = self.by_name.get(name) {
            return *id;
        }
        let id = OperatorId(self.names.len() as u32);
        self.names.push(name.to_string());
        self.by_name.insert(name.to_string(), id);
        id
    }

    /// Look up an existing id without allocating.
    pub fn lookup(&self, name: &str) -> Option<OperatorId> {
        self.by_name.get(name).copied()
    }

    /// Resolve an extension operator display name.
    pub fn name(&self, id: OperatorId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    /// Registered extension count.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
