//! Execution engine handle — hosts compose requests; math lives in submodules.

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::term::Term;

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

    /// Evaluate a bridge [`Term`] under built-in definitions.
    pub fn evaluate_term(&self, expr: &Term) -> Term {
        crate::eval::evaluate(expr)
    }

    /// Differentiate then evaluate.
    pub fn differentiate_term(&self, expr: &Term, var: &str) -> Term {
        crate::eval::evaluate(&crate::calculus::differentiate(expr, var))
    }

    /// Simplify via `Simplify` head.
    pub fn simplify_term(&self, expr: &Term) -> Term {
        self.evaluate_term(&Term::app("Simplify", vec![expr.clone()]))
    }

    /// Arena/`()` stub evaluate — retained until IR path lands.
    pub fn evaluate(&self, _term: &(), _opts: &EvalOptions) -> Result<()> {
        Err(Diagnostic::error(
            DiagnosticCode::UnsupportedOperation,
            "evaluate not yet implemented",
        ))
    }
}
