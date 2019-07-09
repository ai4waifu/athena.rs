//! Cheap structural snapshot of a `TermId` (no numeric payload copy).

use athena_types::{OperatorId, SymbolId, TermId};

/// Cheap structural snapshot (numbers are tagged `Number` without copying payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Shape {
    Number,
    String(String),
    Symbol(SymbolId),
    Bool(bool),
    Null,
    List(Vec<TermId>),
    Application(OperatorId, Vec<TermId>),
}
