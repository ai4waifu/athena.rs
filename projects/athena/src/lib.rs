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
    AlgebraMapId, AlgebraParentId, AssumptionSet, AssumptionSetId, AthenaEngine, Atom, AtomKind, AutomorphismId, BranchPolicy,
    CalculusRequest, CalculusResult, CalculusValue, CanonicalPolynomial, ClosureLimits, ClosureResult, CoefficientDomain,
    CoefficientParent, CofactorStatus, CompositeWitness, Condition, ConditionalResult, CongruenceSolution,
    ConnectedComponentsResult, CrtResult, Curl,
    DerivativeOrder, DeterminacyGuarantee, DeterminacyState, Diagnostic, DiagnosticCode, DiagnosticPath, DiagnosticValue,
    DifferentialSolution, Divergence, DivisionPolicy, DomainId, DomainRef, DomainRequest, DomainResult, EqualityWitness,
    EquivalenceClasses, EvalOptions, ExactNumber, ExactnessLevel, ExtendedGcd, ExtensionId, FactorAlgorithms, FactorBaseStatus,
    FactorComponent, FactorExecutionBudget, FactorFrontier, FactorLimits, FactorPolicy, Factorization,
    FactorizationCompleteness, FactorizationVerifyError, Field, FieldAutomorphism, FieldDescriptor, FieldDomainValue,
    FieldElement, FieldElementRepr, FieldExtension, FieldId, FieldKind, FieldRequest, FieldResult, FieldTable, FunctionDefinition,
    GaloisComputation, GaloisDomainValue, GaloisGroup, GaloisRequest, GaloisResult, Gradient, GraphAssumptions,
    GraphCertificate, GraphHandle, GraphNodeId, GraphObject, GraphPresentation, GraphPropertyKind, GraphPropertyResult,
    GraphPropertyState, GraphProvenance, GraphSemantics, GraphTheoryRequest, GraphTheoryResult, GraphTheoryValue,
    GroebnerAlgorithm, GroebnerBasis, GroebnerBasisValue, GroebnerCertificate, GroebnerComputation, GroebnerFrontier,
    GroebnerLimits, GroebnerStatus, GroebnerVerificationReport, Group, GroupDescriptor, GroupDomainValue, GroupElement,
    GroupElementId, GroupElementRepr, GroupId, GroupKind, GroupPropertyFacts, GroupRequest, GroupResult, GroupTable, Hessian,
    HyperEdge, Ideal, Jacobian, JitParityOutcome, LimitApproach, LimitDirection, MGraphState, MemoryGraph,
    MillerRabinBaseSelection, ModularTimingPolicy, ModularValue, Modulus, ModulusContext, ModulusTable, MonomialOrder,
    MonomialTerm, NodeId, Number, NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue, NumericBackend,
    NumericBackendContract, NumericBackendLimits, NumericCapability, NumericDomain, NumericOperation, NumericResultMode,
    NumericValue, OperatorId, POLYNOMIAL_SOLVER_ID, Permutation, Polynomial, PolynomialBuilder, PolynomialCacheKey,
    PolynomialCacheOp, PolynomialCofactorStatus, PolynomialDomainValue, PolynomialFactorComponent, PolynomialFactorLimits,
    PolynomialFactorStatus, PolynomialFactorization, PolynomialFactorizationCompleteness, PolynomialFingerprint,
    PolynomialMGraphStore, PolynomialRepr, PolynomialReprBody, PolynomialRequest, PolynomialResult, PolynomialValue,
    PolynomialWitness, Precision, Predicate, PresentationId, Primality,
    PrimeCertificate, PrimeIterator, PrimeModulus, ProbablePrimeEvidence, ProbablePrimeModulus, ProofRequirement, PropertyState,
    PureRustBackend, RationalReconstruction, RationalReconstructionFailure, RealNumber, ReflectionResult, Reflector,
    RegionOfConvergence, Remainder, ReprTarget, Residue, Result, RewriteOptions, RewriteResult, RewriteWitness, Rewriter,
    RingCharacteristic, RingDescriptor, RingId, RingTable, RoundingMode, SampleDomain, SamplePoint, SampledCurve, SamplingPolicy,
    SerializationVersion, Series, Session, Severity, ShortestPathResult, SimplifyOptions, SolverCandidate, SolverContext,
    SolverFrontier, SolverId, SolverLimits, SolverMetadata, SolverOperation, SolverRegistry, SolverRequest, SolverScore,
    SourceSpan, SubgroupId, SymbolId, SymbolTable, Term, TermArena, TermBuilder, TermId, TermKind, TransformKind,
    TransformResult, VerificationStatus, VerifiedGroebnerBasis, WeightDomain, WireNumber, add_polynomial,
    apply_field_automorphism, apply_prime_subfield_embedding, asymptotic, batch_mod_inverse, cache_key_for_request,
    calculus_result_bridge_term, canonical_extension_element, canonical_hash, canonical_prime_residue, canonicalize_polynomial,
    chinese_remainder, chinese_remainder_pair, compute_elimination_basis, compute_groebner_basis, curl_checked,
    definite_integrate_checked, differentiate, differentiate_checked, differentiate_term, divergence_checked, evaluate,
    execute_calculus, execute_domain, execute_field, execute_galois, execute_galois_with_tables, execute_graph_theory,
    execute_group, execute_number_theory, execute_polynomial, execute_polynomial_mgraph, execute_polynomial_with_rings,
    extended_gcd, factor_component_from_primality, factor_continue, factor_integer, factor_univariate,
    factorization_to_frontier, fermat_split, dixon_split, qs_split, field_automorphism,
    fourier_checked, frobenius_coords, gcd, gradient_checked, hessian_checked, ideal_membership, integrate, integrate_checked,
    is_galois_extension, is_perfect_power, isqrt, isqrt_if_exact, jacobi_symbol, jacobian_checked, kronecker_symbol,
    laplace_checked, laurent, lcm, limit_checked, lookup_function, mod_inverse, mod_pow, mul_polynomial, mul_with_jit_parity,
    next_prime_after, number_from_term, number_from_wire, parity_diagnostic, perfect_power_decomposition,
    polynomial_canonical_hash, primality_test, primes_up_to, rational_reconstruction, record_polynomial_result,
    reduce_by_verified, reduce_ideal, registered_function_names, reprs_mathematically_equal, residue_checked, run_closure_step,
    sample_1d, score_candidate, solve_linear_congruence, solve_ode_checked, sub_polynomial, taylor, try_calculus_request,
    verify_factorization, verify_groebner_basis, witness_from_exact, z_checked,
};

/// 数值塔：[`NumericValue`] / [`Number`] 为唯一执行真相源。
pub use athena_engine::numeric;
