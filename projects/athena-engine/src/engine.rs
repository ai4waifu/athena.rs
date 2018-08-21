//! Execution engine handle — hosts compose requests; math lives in submodules.

use athena_types::{Diagnostic, DiagnosticCode};

/// Evaluation options (placeholder; expands with modes/session).
#[derive(Debug, Clone, Default)]
pub struct EvalOptions {}

/// Simplification options.
#[derive(Debug, Clone, Default)]
pub struct SimplifyOptions {}

/// Primary Athena engine handle (stateless rules; use [`Session`] for bindings).
#[derive(Debug, Default)]
pub struct AthenaEngine {}

impl AthenaEngine {
    /// Create engine with default operator registry (stub).
    pub fn new() -> Self {
        Self {}
    }

    /// 求值 — stub，待 `eval` + `ir` 落地。
    pub fn evaluate(&self, _term: &(), _opts: &EvalOptions) -> Result<(), Diagnostic> {
        Err(Diagnostic::error(
            DiagnosticCode::UnsupportedOperation,
            "evaluate not yet implemented",
        ))
    }
}
