//! Athena CAS 执行引擎 — 唯一决定「怎么算」的地方。
//!
//! ```text
//! athena-types → athena-ir → athena-rewriter → athena-engine → athena
//! ```
//!
//! 本 crate 拥有求值、Session、改写编排、域分派与 `ATHENA_*` 诊断。
//! 不解析方言、不渲染字符串、也不绑定 N-API/WASM。

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
    CalculusRequest, CalculusResult, CalculusValue, ConditionalResult, Curl, DerivativeOrder, DifferentialSolution,
    Divergence, DomainRequest, Gradient, Hessian, Jacobian, LimitApproach, LimitDirection, RegionOfConvergence, Remainder,
    Residue, Series, TransformKind, TransformResult, VerificationStatus, asymptotic, calculus_result_bridge_term,
    curl_checked, definite_integrate_checked, differentiate, differentiate_checked, divergence_checked, execute_calculus,
    execute_domain, fourier_checked, gradient_checked, hessian_checked, integrate, integrate_checked, jacobian_checked,
    laplace_checked, laurent, limit_checked, residue_checked, solve_ode_checked, taylor, try_calculus_request, z_checked,
};
pub use engine::{AthenaEngine, EvalOptions, SimplifyOptions};
pub use eval::{differentiate as differentiate_term, evaluate};
pub use function::{BranchPolicy, FunctionDefinition, lookup_function, registered_function_names};
pub use session::Session;
pub use term::{Atom, Term, number_from_term};
