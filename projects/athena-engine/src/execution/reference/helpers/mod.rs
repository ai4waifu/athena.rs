//! Pure helpers for [`super::ReferenceExecutor`] (no SSA frame state).

mod arithmetic;
mod compare;
mod terms;

use athena_types::{Diagnostic, DiagnosticCode};

pub(super) use arithmetic::*;
pub(super) use compare::*;
pub(super) use terms::*;

pub(super) fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("component", "ReferenceExecutor").detail("reason", reason)
}
