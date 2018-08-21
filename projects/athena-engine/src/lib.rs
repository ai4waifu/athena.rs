//! Athena CAS execution engine — the only place that decides how math is evaluated.
//!
//! ```text
//! athena-types → athena-ir → athena-rewriter → athena-engine → athena
//! ```
//!
//! This crate owns evaluation, Session, rewrite orchestration, domain dispatch, and
//! `ATHENA_*` diagnostics. It does not parse dialects, render strings, or bind N-API/WASM.

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
pub mod term;

mod engine;

pub use athena_ir::{AtomKind, SymbolTable, TermArena, TermBuilder, TermKind, canonical_hash};
pub use athena_rewriter::{RewriteOptions, RewriteResult, Rewriter};
pub use athena_types::{
    AssumptionSet, AssumptionSetId, Condition, Diagnostic, DiagnosticCode, DomainId, ExactNumber, NodeId, Number,
    NumericDomain, OperatorId, Precision, Predicate, RealNumber, Result, RoundingMode, SerializationVersion, Severity,
    SourceSpan, SymbolId, TermId,
};
pub use calculus::{
    CalculusRequest, CalculusResult, ConditionalResult, DerivativeOrder, DomainRequest, LimitApproach, LimitDirection,
    differentiate, differentiate_checked, execute_calculus, execute_domain, integrate, integrate_checked, limit_checked,
};
pub use engine::{AthenaEngine, EvalOptions, SimplifyOptions};
pub use eval::{evaluate, differentiate as differentiate_term};
pub use session::Session;
pub use term::{Atom, Term, number_from_term};
