//! Athena 公共 Rust 门面 — 对 [`athena_engine`] 的薄且稳定入口。
//!
//! ```text
//! athena-types → athena-ir → athena-rewriter → athena-engine → athena
//! ```
//!
//! 本 crate **不**自行实现求值或 Session。它为普通 Rust 消费者再导出执行引擎与
//! 选定的 IR/类型合同。宿主（如 SXO）应依赖本 crate，而非直接依赖 `athena-engine`。

#![deny(missing_docs)]

pub use athena_engine::{
    AssumptionSet, AssumptionSetId, AthenaEngine, Atom, AtomKind, CalculusRequest, CalculusResult, CalculusValue, Condition,
    ConditionalResult, Curl, DerivativeOrder, Diagnostic, DiagnosticCode, DifferentialSolution, Divergence, DomainId,
    DomainRequest, EvalOptions, ExactNumber, Gradient, Hessian, Jacobian, LimitApproach, LimitDirection, NodeId, Number,
    NumericDomain, OperatorId, Precision, Predicate, RealNumber, RegionOfConvergence, Remainder, Result, RewriteOptions,
    RewriteResult, Rewriter, RoundingMode, SerializationVersion, Series, Session, Severity, SimplifyOptions, SourceSpan,
    SymbolId, SymbolTable, Term, TermArena, TermBuilder, TermId, TermKind, TransformKind, TransformResult, VerificationStatus,
    calculus_result_bridge_term, canonical_hash, curl_checked, definite_integrate_checked, differentiate,
    differentiate_checked, differentiate_term, divergence_checked, evaluate, execute_calculus, execute_domain,
    fourier_checked, gradient_checked, hessian_checked, integrate, integrate_checked, jacobian_checked, laplace_checked,
    limit_checked, number_from_term, solve_ode_checked, taylor, try_calculus_request, z_checked,
};
