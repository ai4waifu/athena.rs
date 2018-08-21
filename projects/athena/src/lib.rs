//! Athena public Rust facade — thin, stable entry over [`athena_engine`].
//!
//! ```text
//! athena-types → athena-ir → athena-rewriter → athena-engine → athena
//! ```
//!
//! This crate does **not** re-implement evaluation or Session. It re-exports the
//! execution engine and selected IR/types contracts for ordinary Rust consumers.
//! Hosts such as SXO should depend on this crate, not on `athena-engine` directly.

#![deny(missing_docs)]

pub use athena_engine::{
    AthenaEngine, AtomKind, Diagnostic, DiagnosticCode, DomainId, ExactNumber, EvalOptions, NodeId, Number,
    NumericDomain, OperatorId, Precision, RealNumber, Result, RewriteOptions, RewriteResult, Rewriter,
    RoundingMode, SerializationVersion, Session, Severity, SimplifyOptions, SourceSpan, SymbolId, SymbolTable,
    TermArena, TermBuilder, TermId, TermKind, canonical_hash,
};
