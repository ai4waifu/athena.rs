//! [`super::ReferenceExecutor`] 的纯辅助（无 SSA 帧状态）。

mod arithmetic;
mod compare;
mod index;
mod structure;
mod terms;
mod unary;

use athena_types::{Diagnostic, DiagnosticCode};

pub(crate) use arithmetic::*;
pub(crate) use compare::*;
pub(crate) use index::*;
pub(crate) use structure::*;
pub(crate) use terms::*;
pub(crate) use unary::*;

pub(super) fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("component", "ReferenceExecutor").detail("reason", reason)
}
