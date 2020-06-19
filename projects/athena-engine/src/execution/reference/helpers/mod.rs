//! [`super::ReferenceExecutor`] 的纯辅助（无 SSA 帧状态）。

mod apply;
mod arithmetic;
mod compare;
mod index;
mod iterator;
mod map;
mod structure;
mod terms;
mod unary;

use athena_types::{Diagnostic, DiagnosticCode};

pub(crate) use apply::*;
pub(crate) use arithmetic::*;
pub(crate) use compare::*;
pub(crate) use index::*;
pub(crate) use iterator::*;
pub(crate) use map::*;
pub(crate) use structure::*;
pub(crate) use terms::*;
pub(crate) use unary::*;

pub(super) fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("component", "ReferenceExecutor").detail("reason", reason)
}
