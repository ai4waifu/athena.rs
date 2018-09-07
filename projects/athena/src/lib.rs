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
    AssumptionSet, AssumptionSetId, AthenaEngine, Atom, AtomKind, BranchPolicy, CalculusRequest, CalculusResult, CalculusValue,
    Condition, ConditionalResult, Curl, DerivativeOrder, Diagnostic, DiagnosticCode, DifferentialSolution, Divergence,
    DomainId, DomainRequest, DomainResult, EvalOptions, ExactNumber, ExtendedGcd, FactorLimits, Factorization,
    FactorizationCompleteness, FunctionDefinition, Gradient, Hessian, Jacobian, LimitApproach, LimitDirection, ModularValue,
    Modulus, NodeId, Number, NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue, NumericDomain, OperatorId,
    Precision, Predicate, Primality, PrimePower, RealNumber, RegionOfConvergence, Remainder, Residue, Result, RewriteOptions,
    RewriteResult, Rewriter, RoundingMode, SerializationVersion, Series, Session, Severity, SimplifyOptions, SourceSpan,
    SymbolId, SymbolTable, Term, TermArena, TermBuilder, TermId, TermKind, TransformKind, TransformResult, VerificationStatus,
    asymptotic, calculus_result_bridge_term, canonical_hash, curl_checked, definite_integrate_checked, differentiate,
    differentiate_checked, differentiate_term, divergence_checked, evaluate, execute_calculus, execute_domain,
    execute_number_theory, extended_gcd, factor_integer, fourier_checked, gcd, gradient_checked, hessian_checked, integrate,
    integrate_checked, jacobian_checked, laplace_checked, laurent, lcm, limit_checked, lookup_function, mod_inverse, mod_pow,
    number_from_term, primality_test, registered_function_names, residue_checked, solve_ode_checked, taylor,
    try_calculus_request, z_checked,
};
