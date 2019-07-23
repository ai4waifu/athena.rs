//! Source-independent module fingerprint (not `TermId` indices or renderer text).

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use super::module::ExecutionModule;

/// Stable fingerprint over structural module content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleFingerprint(pub u64);

impl ModuleFingerprint {
    /// Compute a structural fingerprint.
    ///
    /// Uses a dedicated seed so default hasher instability across processes is
    /// not silently treated as a cache key. Callers that need cross-process
    /// stability must replace this with a fixed-byte canonical encode later.
    pub fn of_module(module: &ExecutionModule) -> Self {
        let mut hasher = DefaultHasher::new();
        0x4154_4845_4e41_4558u64.hash(&mut hasher); // "ATHENAEX"
        module.inputs.len().hash(&mut hasher);
        module.constants.len().hash(&mut hasher);
        module.captured_roots.len().hash(&mut hasher);
        module.regions.len().hash(&mut hasher);
        module.effect_edges.len().hash(&mut hasher);
        module.exits.len().hash(&mut hasher);
        module.provider_calls.len().hash(&mut hasher);
        for region in &module.regions {
            region.id.0.hash(&mut hasher);
            region.entry.0.hash(&mut hasher);
            region.blocks.len().hash(&mut hasher);
            for block in &region.blocks {
                block.id.0.hash(&mut hasher);
                block.parameters.len().hash(&mut hasher);
                block.operations.len().hash(&mut hasher);
            }
        }
        Self(hasher.finish())
    }
}
