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
    AlgebraElement, AlgebraMapId, AlgebraParentId, AssumptionSet, AssumptionSetId, AthenaEngine, Atom, AtomKind, Automorphism,
    AutomorphismId, BranchPolicy, CalculusRequest, CalculusResult, CalculusValue, ClosureLimits, ClosureResult, CoefficientDomain,
    CoefficientParent, Condition, ConditionalResult, Curl, DerivativeOrder, DeterminacyGuarantee, DeterminacyState, Diagnostic,
    DiagnosticCode, DiagnosticPath, DiagnosticValue, DifferentialSolution, Divergence, DivisionPolicy, DomainId, DomainRef,
    DomainRequest, DomainResult, EqualityWitness, EquivalenceClasses, EvalOptions, ExactNumber, ExactnessLevel, ExtendedGcd,
    ExtensionId, FactorLimits, Factorization, FactorizationCompleteness, Field, FieldAutomorphism, FieldDomainValue, FieldElement,
    FieldElementRepr, FieldId, FieldKind, FieldPresentation, FieldRequest, FieldResult, FieldTable, FunctionDefinition,
    GaloisComputation, GaloisDomainValue, GaloisGroup, GaloisRequest, GaloisResult, Gradient, Group, GroupDescriptor,
    GroupDomainValue, GroupElement, GroupElementId, GroupElementRepr, GroupId, GroupPresentation, GroupPropertyFacts, GroupRequest,
    GroupResult, Hessian, HyperEdge, Jacobian, LimitApproach, LimitDirection, MGraphState, ModularValue, Modulus, MonomialOrder,
    MonomialTerm, NodeId, Number, NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue, NumericBackend,
    NumericBackendContract, NumericBackendLimits, NumericCapability, NumericDomain, NumericOperation, NumericResultMode,
    NumericValue, OperatorId, Permutation, Polynomial, PolynomialBuilder, CanonicalPolynomial, GroebnerAlgorithm, GroebnerBasis,
    GroebnerBasisValue, GroebnerCertificate, GroebnerLimits, Ideal, JitParityOutcome, MillerRabinBaseSelection, MillerRabinEvidence,
    PolynomialCacheKey, PolynomialCacheOp,
    PolynomialDomainValue, PolynomialMGraphStore, PolynomialWitness, POLYNOMIAL_SOLVER_ID, PolynomialRepr, PolynomialReprBody,
    PolynomialRequest, PolynomialResult, PolynomialValue, Precision, Predicate, PresentationId, Primality, PrimePower,
    PropertyState, PureRustBackend, RealNumber, ReflectionResult, Reflector, RegionOfConvergence, Remainder, Residue, Result,
    RewriteOptions, RewriteResult, RewriteWitness, Rewriter, RingCharacteristic, RingDescriptor, RingId, RingTable, RoundingMode,
    SampleDomain, SamplePoint, SampledCurve, SamplingPolicy, SerializationVersion, Series, Session, Severity, SimplifyOptions,
    SolverCandidate, SolverContext, SolverFrontier, SolverId, SolverLimits, SolverMetadata, SolverOperation, SolverRegistry,
    SolverRequest, SolverScore, SourceSpan, SubgroupId, SymbolId, SymbolTable, Term, TermArena, TermBuilder, TermId, TermKind,
    TransformKind, TransformResult, VerificationStatus, WireNumber, add_polynomial, asymptotic, calculus_result_bridge_term,
    canonical_hash, cache_key_for_request, compute_elimination_basis, compute_groebner_basis, canonicalize_polynomial, curl_checked,
    definite_integrate_checked, differentiate, differentiate_checked, differentiate_term, divergence_checked, evaluate,
    execute_calculus, execute_domain, execute_field, execute_galois, execute_group, execute_number_theory, execute_polynomial,
    execute_polynomial_mgraph, execute_polynomial_with_rings, extended_gcd, factor_integer, fourier_checked, gcd, gradient_checked,
    hessian_checked, integrate, integrate_checked, jacobian_checked, laplace_checked, laurent, lcm, limit_checked,
    lookup_function, mod_inverse, mod_pow, mul_polynomial, mul_with_jit_parity, number_from_term, number_from_wire,
    parity_diagnostic, polynomial_canonical_hash, primality_test, reduce_ideal, registered_function_names, record_polynomial_result,
    ReprTarget, reprs_mathematically_equal, residue_checked, run_closure_step, sub_polynomial, witness_from_exact, sample_1d,
    score_candidate, solve_ode_checked, taylor, try_calculus_request, z_checked,
};

/// 数值塔（Living `16`：[`NumericValue`] / [`Number`] 为唯一执行真相源）。
pub use athena_engine::numeric;
