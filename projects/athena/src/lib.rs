//! Pure Rust CAS kernel.
//!
//! ```text
//! athena-types → athena-ir → athena-rewriter → athena
//! ```

#![deny(missing_docs)]

pub mod calculus;
pub mod domain;
pub mod eval;
pub mod function;
pub mod ir;
pub mod object;
pub mod rewriter;
pub mod session;
pub mod symbol;

mod engine;

pub use athena_ir::{AtomKind, SymbolTable, TermArena, TermBuilder, TermKind, canonical_hash};
pub use athena_rewriter::{RewriteOptions, RewriteResult, Rewriter};
pub use athena_types::{
    Diagnostic, DiagnosticCode, DomainId, ExactNumber, NodeId, Number, NumericDomain, OperatorId, Precision, RealNumber,
    Result, RoundingMode, SerializationVersion, Severity, SourceSpan, SymbolId, TermId,
};
pub use engine::{EvalOptions, SimplifyOptions, athenaEngine};
pub use session::Session;
