//! Athena CAS 执行引擎 — 唯一决定「怎么算」的地方。
//!
//! ```text
//! athena-types → athena-numeric → athena-ir → athena-rewriter → athena-engine → athena
//! ```
//!
//! 本 crate 拥有求值、Session、M-Graph、solver、改写编排、域分派与 `ATHENA_*` 诊断。
//! 不解析方言、不渲染字符串、也不绑定 N-API/WASM。

#![deny(missing_docs)]

pub mod algebra;
pub mod calculus;
pub mod domain;
pub mod eval;
pub mod field;
pub mod function;
pub mod galois;
pub mod graph_theory;
pub mod group;
pub mod ir;
pub mod linear_algebra;
pub mod mgraph;
pub mod number_theory;
pub mod object;
pub mod optimization;
pub mod plot;
pub mod polynomial;
pub mod rewriter;
pub mod semantic;
pub mod session;
pub mod solve;
pub mod solver;
pub mod symbol;
pub mod term;

mod engine;

/// 数值塔：[`NumericValue`] 为唯一执行真相源。
pub use athena_numeric as numeric;

pub use algebra::{
    AlgebraElement, AlgebraMap, AlgebraMapKind, AlgebraParentId, BsgsChain, CoefficientParent, ElementProvenance,
    FieldEmbedding, FieldExtension, FieldFingerprint, FieldPresentation, FieldPresentationFingerprint, FieldPresentationKind,
    FieldTable, FiniteFieldPolySpec, GroupFingerprint, GroupHomomorphism, GroupPresentation, GroupPresentationFingerprint,
    GroupPresentationKind, GroupPropertyFacts, GroupTable, MapTable, MapVerification, MapVerificationKind, NumberFieldSpec,
    PermutationGroupSpec, PropertyState, PropertyWitness, QuotientProjection, SubgroupInclusion, field_automorphism,
    frobenius_coords, is_galois_extension,
};
pub use athena_ir::{AtomKind, SymbolTable, TermArena, TermBuilder, TermKind, canonical_hash};
pub use athena_numeric::{
    BarrettParams, ExactInteger, ExactRational, Integer, ModularTimingPolicy, ModularValue, Modulus, ModulusBinding,
    ModulusContext, ModulusTable, MontgomeryParams, Number, NumericBackend, NumericBackendContract, NumericBackendLimits,
    NumericCapability, NumericDomain, NumericOperation, NumericResultMode, NumericValue, PrimeModulus, ProbablePrimeModulus,
    PureRustBackend, Rational, number_from_wire,
};
pub use athena_rewriter::{RewriteOptions, RewriteResult, Rewriter};
pub use athena_types::{
    AlgebraMapId, AssumptionBranchPolicy, AssumptionScope, AssumptionScopeId, AssumptionSet, AssumptionSetId, AutomorphismId,
    CoefficientRingId, ComputationStatus, Condition, Diagnostic, DiagnosticCode, DiagnosticPath, DiagnosticValue, DomainId,
    ExprId, ExtensionId, FieldId, FieldPresentationId, FormId, GroupElementId, GroupId, GroupPresentationId, MatrixId, NodeId,
    OperatorId, PolynomialId, Precision, Predicate, PresentationId, ProofRef, Result, ResultId, RoundingMode,
    ScopeApplicability, ScopeConflict, ScopeConflictKind, ScopeMergeOutcome, SerializationVersion, Severity, SourceSpan,
    SubgroupId, SymbolId, TermId, TheoryContext, TheoryContextId, ValueId,
    wire::{ExactNumber, RealNumber, WireNumber},
};
pub use calculus::{
    CalculusRequest, CalculusResult, CalculusValue, ConditionalResult, Curl, DerivativeOrder, DifferentialSolution, Divergence,
    Gradient, Hessian, Jacobian, LimitApproach, LimitDirection, RegionOfConvergence, Remainder, Residue, Series, TransformKind,
    TransformResult, VerificationStatus, asymptotic, calculus_result_bridge_term, curl_checked, definite_integrate_checked,
    differentiate, differentiate as differentiate_term, differentiate_checked, divergence_checked, execute_calculus,
    fourier_checked, gradient_checked, hessian_checked, integrate, integrate_checked, jacobian_checked, laplace_checked,
    laurent, limit_checked, residue_checked, solve_ode_checked, taylor, try_calculus_request, z_checked,
};
pub use domain::{DomainRequest, DomainResult, execute_domain};
pub use engine::{AthenaEngine, EvalOptions, SimplifyOptions};
pub use eval::evaluate;
pub use field::{
    Field, FieldDescriptor, FieldDomainValue, FieldElement, FieldElementRepr, FieldKind, FieldRequest, FieldResult,
    add_field_elements, apply_base_field_embedding, apply_field_automorphism, apply_field_embedding,
    apply_prime_subfield_embedding, canonical_extension_element, canonical_number_field_element, canonical_prime_residue,
    canonical_rational, execute_field, execute_field_with_table, execute_field_with_table_mut, inv_field_element,
    mul_field_elements,
};
pub use function::{BranchPolicy, FunctionDefinition, lookup_function, registered_function_names};
pub use galois::{
    Automorphism, FieldAutomorphism, GaloisComputation, GaloisDomainValue, GaloisGroup, GaloisRequest, GaloisResult,
    execute_galois, execute_galois_with_tables,
};
pub use graph_theory::{
    BipartiteResult, CertificateStrength, ConnectedComponentsResult, GraphAssumptions, GraphCertificate, GraphHandle, GraphId,
    GraphNodeId, GraphObject, GraphPresentation, GraphPropertyKind, GraphPropertyResult, GraphPropertyState, GraphProvenance,
    GraphRevision, GraphSemantics, GraphSnapshot, GraphTheoryRequest, GraphTheoryResult, GraphTheoryValue, MemoryGraph,
    MinimumSpanningForestResult, RepresentationId, ShortestPathResult, SpanningEdge, StronglyConnectedComponentsResult,
    WeightDomain, execute_graph_theory, operation_name,
};
pub use group::{
    Group, GroupDescriptor, GroupDomainValue, GroupElement, GroupElementRepr, GroupKind, GroupRequest, GroupResult,
    Permutation, Subgroup, apply_group_homomorphism, canonical_permutation, execute_group, execute_group_with_table,
    execute_group_with_table_mut, group_membership, inverse_group_element, multiply_group_elements, project_quotient_element,
};
pub use linear_algebra::{
    AlgorithmGuarantee, AxisRange, DEFAULT_PIVOT_THRESHOLD, DialectArgs, DialectMatrixOp, DialectOrigin, ElementParentKind,
    ExactDetResult, ExactRankResult, ExactRrefResult, ExactSolveResult, IndexSpec, Layout, LinearAlgebraRequest,
    LinearAlgebraResult, LinearAlgebraValue, MachineLuFactorization, MachineSolveResult, MachineSolveWitness, MatrixBuffer,
    MatrixEntry, MatrixEqualityKind, MatrixParent, MatrixShape, MatrixValue, RoundingPolicy, ShapePolicy, SolveDisposition,
    SparseStrategy, StorageOrder, det_bareiss, execute_linear_algebra, hadamard, index_scalar, lower_1based_inclusive_slice,
    lower_1based_scalar, lower_dialect_op, lu_partial_pivot, matlab_star_kind, matmul, matrices_equal,
    operation_name as linear_algebra_operation_name, rank_exact, rank_machine, rref_rational, slice_matrix, solve_exact,
    solve_lu, solve_machine, transpose,
};
pub use mgraph::{
    AdmissionGate, AdmissionOutcome, AdmissionRejectReason, Claim, ClosureLimits, ClosureResult, ClosureSeeds, DerivedIndexes,
    DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, Evidence, EvidenceVerifier, ExactUnionFind,
    ExactnessLevel, FactId, FactLog, Guarantee, HyperEdge, MGraphCore, MGraphState, MGraphView, OperationalState,
    OuterCandidate, POLYNOMIAL_SOLVER_ID, PolynomialCacheEntry, PolynomialCacheTier, PolynomialMGraphStore, PolynomialWitness,
    Proposition, RelationRecord, RelationRef, RelationStatus, ResultCache, RewriteWitness, Scope, ScopeEdge, ScopeRef,
    ScopeRelationKind, SemanticCore, SolverCandidate, SolverFrontier, SolverId, SolverScore, VerificationPolicy, VerifiedClaim,
    admit_polynomial_exact, admit_polynomial_result, is_admitted, proposition_from_cache_key, run_closure_step, scope_from_ref,
    scope_to_ref, witness_from_exact,
};
pub use number_theory::{
    CofactorStatus, CompositeWitness, CongruenceSolution, CrtResult, ExtendedGcd, FactorAlgorithms, FactorBaseStatus,
    FactorComponent, FactorExecutionBudget, FactorFrontier, FactorLimits, FactorPolicy, FactorProducer, Factorization,
    FactorizationCompleteness, FactorizationVerifyError, MillerRabinBaseSelection, NumberTheoryRequest, NumberTheoryResult,
    NumberTheoryValue, Primality, PrimeCertificate, PrimeIterator, ProbablePrimeEvidence, ProofRequirement,
    PureRustFactorProducer, RationalReconstruction, RationalReconstructionFailure, batch_mod_inverse, chinese_remainder,
    chinese_remainder_pair, dixon_split, execute_number_theory, extended_gcd, factor_component_from_primality, factor_continue,
    factor_continue_with_producer, factor_integer, factor_integer_with_producer, factorization_to_frontier, fermat_split, gcd,
    is_perfect_power, isqrt, isqrt_if_exact, jacobi_symbol, kronecker_symbol, lcm, mod_inverse, mod_inverse_with_table,
    mod_pow, mod_pow_with_table, next_prime_after, perfect_power_decomposition, primality_test, primes_up_to, qs_split,
    rational_reconstruction, solve_linear_congruence, verify_factorization,
};
pub use optimization::{
    AlgorithmPolicy, BoundCertificate, CertificateKind, ClosureStatus, Constraint as OptimizationConstraint, ConstraintId,
    ConstraintRelation, DecisionVariable, FINGERPRINT_ALGORITHM as OPTIMIZATION_FINGERPRINT_ALGORITHM, FeasibleSet,
    Integrality, Objective, ObjectiveId, ObjectiveSense, OptimalityKind, OptimizationFingerprint, OptimizationFrontier,
    OptimizationLimits, OptimizationProblem, OptimizationRequest, OptimizationResult, ProblemClass, ProblemId, VariableDomain,
    VariableId, VariableMetadata, execute_optimization, fingerprint_placeholder as optimization_fingerprint_placeholder,
    operation_name as optimization_operation_name,
};
pub use plot::{SampleDomain, SamplePoint, SampledCurve, SamplingPolicy, sample_1d};
pub use polynomial::{
    CanonicalPolynomial, CoeffRing, CoeffRingTable, CoefficientDomain, CoefficientRingDescriptor, DivisionPolicy,
    FINGERPRINT_ALGORITHM, FpBigKernel, FpWordKernel, GroebnerAlgorithm, GroebnerBasis, GroebnerBasisValue,
    GroebnerCertificate, GroebnerComputation, GroebnerFrontier, GroebnerLimits, GroebnerStatus, GroebnerVerificationReport,
    Ideal, JitParityOutcome, MonomialLayout, MonomialOrder, MonomialTerm, PackedMonomial, Polynomial, PolynomialBuilder,
    PolynomialCacheKey, PolynomialCacheOp, PolynomialCofactorStatus, PolynomialDomainValue, PolynomialFactorComponent,
    PolynomialFactorLimits, PolynomialFactorStatus, PolynomialFactorization, PolynomialFactorizationCompleteness,
    PolynomialFingerprint, PolynomialRepr, PolynomialReprBody, PolynomialRequest, PolynomialResult, PolynomialValue,
    QCoeffKernel, ReprTarget, RingCharacteristic, RingDescriptor, RingFingerprint, RingHandle, RingId, RingTable,
    SpecializedCoeffKernel, UnivariateDivision, UnivariateDivisionValue, VerifiedGroebnerBasis, ZCoeffKernel, add_polynomial,
    cache_key_for_request, canonicalize_polynomial, compute_elimination_basis, compute_groebner_basis, div_univariate,
    execute_polynomial, execute_polynomial_mgraph, execute_polynomial_with_rings, factor_univariate, fnv1a64, gcd_univariate,
    ideal_membership, mul_polynomial, mul_with_jit_parity, parity_diagnostic, polynomial_canonical_hash,
    polynomial_fingerprint, polynomial_fingerprint_u64, record_polynomial_result, reduce_by_verified, reduce_ideal,
    reprs_mathematically_equal, resultant_univariate, sub_polynomial, verify_groebner_basis,
};
pub use semantic::{AssumptionScopeTable, ExprBindingTable, ResultIdTable, ValueIdTable};
pub use session::Session;
pub use solve::{
    BindingId, BindingMap, BindingValue, BindingValueTable, BoundSymbol, BranchStatus, Constraint, ConstraintConnective,
    ConstraintSet, CoverageStatus, Equation, ExecutionLimits, Inequality, InequalityOp, LinearAdaptedSolution, LinearSolveMode,
    MultiplicityInfo, QuantifiedConstraint, Quantifier, RelationalOperators, ResidualCertificate, ResumeToken, SolutionBranch,
    SolutionSet, SolveDomain, SolveGoal, SolvePolicy, SolvePredicate, SolveProblem, SolveRelationKind,
    UnivariateAdaptedSolution, adapt_exact_linear_solve, adapt_machine_linear_solve, adapt_univariate_factorization,
    assemble_solve_problem, coverage_from_exact_disposition, coverage_from_factorization, coverage_from_machine_disposition,
    execute_linear_system_goal, execute_polynomial_root_goal, normalize_constraint_conjunction, normalize_relational_app,
    proof_ref_from_witness, require_goal, solve_linear_system_exact, solve_linear_system_machine,
    solve_univariate_polynomial_roots,
};
pub use solver::{
    DomainRef, ReflectionResult, Reflector, SolverContext, SolverLimits, SolverMetadata, SolverOperation, SolverRegistry,
    SolverRequest, score_candidate,
};
pub use term::{Atom, Term, number_from_term};
