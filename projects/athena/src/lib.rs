//! Athena 公共 Rust 门面 — 对 [`athena_engine`] 的薄且稳定入口。
//!
//! ```text
//! athena-types → athena-numeric → athena-ir → athena-rewriter → athena-engine → athena
//! ```
//!
//! 本 crate **不**自行实现求值或 Session。它为普通 Rust 消费者再导出执行引擎与
//! 选定的 IR/类型合同。宿主（如 SXO）应依赖本 crate，而非直接依赖 `athena-engine`。

#![deny(missing_docs)]

pub use athena_engine::{
    AssumptionSet, AssumptionSetId, AthenaEngine, Atom, AtomKind, Automorphism, BranchPolicy, CalculusRequest, CalculusResult,
    CalculusValue, ClosureLimits, ClosureResult, CoefficientRing, Condition, ConditionalResult, Curl, DerivativeOrder,
    DeterminacyGuarantee, DeterminacyState, Diagnostic, DiagnosticCode, DiagnosticPath, DiagnosticValue, DifferentialSolution,
    Divergence, DivisionPolicy, DomainId, DomainRef, DomainRequest, DomainResult, EqualityWitness, EquivalenceClasses,
    EvalOptions, ExactNumber, ExactnessLevel, ExtendedGcd, ExtensionId, FactorLimits, Factorization, FactorizationCompleteness,
    Field, FieldDomainValue, FieldElement, FieldId, FieldKind, FieldRequest, FieldResult, FunctionDefinition,
    GaloisDomainValue, GaloisGroup, GaloisRequest, GaloisResult, Gradient, Group, GroupDomainValue, GroupElement,
    GroupElementId, GroupElementRepr, GroupId, GroupKind, GroupRequest, GroupResult, Hessian, HyperEdge, Jacobian,
    LimitApproach, LimitDirection, MGraphState, ModularValue, Modulus, MonomialTerm, NodeId, Number, NumberTheoryRequest,
    NumberTheoryResult, NumberTheoryValue, NumericBackend, NumericBackendContract, NumericBackendLimits, NumericCapability,
    NumericDomain, NumericOperation, NumericResultMode, NumericValue, OperatorId, Permutation, Polynomial,
    PolynomialDomainValue, PolynomialRequest, PolynomialResult, PolynomialValue, Precision, Predicate, Primality, PrimePower,
    PureRustBackend, RealNumber, ReflectionResult, Reflector, RegionOfConvergence, Remainder, Residue, Result, RewriteOptions,
    RewriteResult, RewriteWitness, Rewriter, RoundingMode, SampleDomain, SamplePoint, SampledCurve, SamplingPolicy,
    SerializationVersion, Series, Session, Severity, SimplifyOptions, SolverCandidate, SolverContext, SolverFrontier, SolverId,
    SolverLimits, SolverMetadata, SolverOperation, SolverRegistry, SolverRequest, SolverScore, SourceSpan, SymbolId,
    SymbolTable, Term, TermArena, TermBuilder, TermId, TermKind, TransformKind, TransformResult, VerificationStatus,
    WireNumber, asymptotic, calculus_result_bridge_term, canonical_hash, curl_checked, definite_integrate_checked,
    differentiate, differentiate_checked, differentiate_term, divergence_checked, evaluate, execute_calculus, execute_domain,
    execute_field, execute_galois, execute_group, execute_number_theory, execute_polynomial, extended_gcd, factor_integer,
    fourier_checked, gcd, gradient_checked, hessian_checked, integrate, integrate_checked, jacobian_checked, laplace_checked,
    laurent, lcm, limit_checked, lookup_function, mod_inverse, mod_pow, number_from_term, number_from_wire, primality_test,
    registered_function_names, residue_checked, run_closure_step, sample_1d, score_candidate, solve_ode_checked, taylor,
    try_calculus_request, z_checked,
};

/// 数值塔（Living `16`：[`NumericValue`] / [`Number`] 为唯一执行真相源）。
pub use athena_engine::numeric;
