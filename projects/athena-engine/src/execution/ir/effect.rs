//! Effect kinds and ordered effect chains.

use super::ids::EffectToken;

/// Kind of observable runtime effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    /// Read Session / scope binding.
    ReadBinding,
    /// Write Session / scope binding.
    WriteBinding,
    /// Enter lexical or dynamic scope.
    EnterScope,
    /// Exit lexical or dynamic scope.
    ExitScope,
    /// Call a typed provider.
    CallProvider,
    /// Publish into `ResultStore`.
    PublishResult,
    /// Explicit GC safepoint.
    Safepoint,
    /// Budget checkpoint.
    BudgetCheck,
    /// Cancellation checkpoint.
    CancellationCheck,
}

/// One link in the module effect chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectEdge {
    /// Token produced / consumed by this edge.
    pub token: EffectToken,
    /// Predecessor token (entry edges use `None`).
    pub precedes_from: Option<EffectToken>,
    /// Effect classification.
    pub kind: EffectKind,
}

impl EffectEdge {
    /// Entry effect with no predecessor.
    pub fn entry(token: EffectToken, kind: EffectKind) -> Self {
        Self { token, precedes_from: None, kind }
    }

    /// Ordered successor effect.
    pub fn after(token: EffectToken, precedes_from: EffectToken, kind: EffectKind) -> Self {
        Self { token, precedes_from: Some(precedes_from), kind }
    }
}
