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
    AssumptionSet, AssumptionSetId, AthenaEngine, Atom, AtomKind, CalculusRequest, CalculusResult, CalculusValue,
    Condition, ConditionalResult, DerivativeOrder, Diagnostic, DiagnosticCode, DomainId, DomainRequest, ExactNumber,
    EvalOptions, LimitApproach, LimitDirection, NodeId, Number, NumericDomain, OperatorId, Precision, Predicate,
    RealNumber, Remainder, Result, RewriteOptions, RewriteResult, Rewriter, RoundingMode, SerializationVersion, Series,
    Session, Severity, SimplifyOptions, SourceSpan, SymbolId, SymbolTable, Term, TermArena, TermBuilder, TermId,
    TermKind, canonical_hash, definite_integrate_checked, differentiate, differentiate_checked, differentiate_term,
    evaluate, execute_calculus, execute_domain, integrate, integrate_checked, limit_checked, number_from_term, taylor,
};
