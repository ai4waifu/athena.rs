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
pub mod group;
pub mod ir;
pub mod mgraph;
pub mod number_theory;
pub mod object;
pub mod plot;
pub mod polynomial;
pub mod rewriter;
pub mod session;
pub mod solver;
pub mod symbol;
pub mod term;

mod engine;

/// 数值塔（Living `16`：[`NumericValue`] 为唯一执行真相源）。
pub use athena_numeric as numeric;

pub use algebra::{
    AlgebraElement, AlgebraMap, AlgebraMapKind, AlgebraParentId, CoefficientParent, ElementProvenance, FieldEmbedding,
    FieldPresentation, FieldPresentationId, FieldPresentationKind, FieldTable, GroupHomomorphism, GroupPresentation,
    GroupPresentationId, GroupPresentationKind, MapTable, GroupPropertyFacts, MapVerification, MapVerificationKind, PropertyState,
    PropertyWitness,
};
pub use athena_ir::{AtomKind, SymbolTable, TermArena, TermBuilder, TermKind, canonical_hash};
pub use athena_numeric::{
    ExactInteger, ExactRational, Integer, ModularTimingPolicy, ModularValue, Modulus, ModulusBinding, ModulusContext,
    ModulusTable, MontgomeryParams, BarrettParams, Number,
    NumericBackend, NumericBackendContract, NumericBackendLimits, NumericCapability, NumericDomain, NumericOperation,
    NumericResultMode, NumericValue, PrimeModulus, ProbablePrimeModulus, PureRustBackend, number_from_wire,
};
pub use athena_rewriter::{RewriteOptions, RewriteResult, Rewriter};
pub use athena_types::{
    AlgebraMapId, AssumptionSet, AssumptionSetId, AutomorphismId, CoefficientRingId, Condition, Diagnostic, DiagnosticCode,
    DiagnosticPath, DiagnosticValue, DomainId, ExtensionId, FieldId, GroupElementId, GroupId, NodeId, OperatorId, Precision,
    Predicate, PresentationId, Result, RoundingMode, SerializationVersion, Severity, SourceSpan, SubgroupId, SymbolId, TermId,
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
    add_field_elements, apply_field_embedding, canonical_prime_residue, canonical_rational, execute_field,
    execute_field_with_table, execute_field_with_table_mut, inv_field_element, mul_field_elements,
};
pub use function::{BranchPolicy, FunctionDefinition, lookup_function, registered_function_names};
pub use galois::{
    Automorphism, FieldAutomorphism, GaloisComputation, GaloisDomainValue, GaloisGroup, GaloisRequest, GaloisResult, execute_galois,
};
pub use group::{
    Group, GroupDescriptor, GroupDomainValue, GroupElement, GroupElementRepr, GroupKind, GroupRequest, GroupResult, Permutation,
    execute_group,
};
pub use mgraph::{
    AdmissionGate, AdmissionOutcome, AdmissionRejectReason, Claim, ClosureLimits, ClosureResult, DerivedIndexes,
    DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, Evidence, EvidenceVerifier, ExactUnionFind,
    ExactnessLevel, FactId, FactLog, Guarantee, HyperEdge, MGraphState, OperationalState, PolynomialCacheEntry,
    PolynomialCacheTier, PolynomialMGraphStore, PolynomialWitness, Proposition, POLYNOMIAL_SOLVER_ID, ResultCache,
    RewriteWitness, Scope, SemanticCore, SolverCandidate, SolverFrontier, SolverId, SolverScore, VerificationPolicy,
    VerifiedClaim, admit_polynomial_exact, admit_polynomial_result, is_admitted, proposition_from_cache_key, run_closure_step,
    witness_from_exact,
};
pub use number_theory::{
    CofactorStatus, CompositeWitness, CongruenceSolution, CrtResult, ExtendedGcd, FactorAlgorithms, FactorBaseStatus,
    FactorComponent, FactorExecutionBudget, FactorFrontier, FactorLimits, FactorPolicy, FactorProducer, Factorization,
    FactorizationCompleteness, FactorizationVerifyError, MillerRabinBaseSelection, NumberTheoryRequest, NumberTheoryResult,
    NumberTheoryValue, Primality, PrimeCertificate, PrimeIterator, ProbablePrimeEvidence, ProofRequirement,
    PureRustFactorProducer, RationalReconstruction, RationalReconstructionFailure, chinese_remainder,
    chinese_remainder_pair, execute_number_theory, extended_gcd, factor_component_from_primality, factor_continue,
    factor_continue_with_producer, factor_integer, factor_integer_with_producer, factorization_to_frontier, gcd,
    is_perfect_power, isqrt, isqrt_if_exact, jacobi_symbol, kronecker_symbol, lcm, mod_inverse, mod_inverse_with_table,
    mod_pow, mod_pow_with_table, batch_mod_inverse, next_prime_after, perfect_power_decomposition, primality_test,
    primes_up_to, rational_reconstruction, solve_linear_congruence, verify_factorization,
};
pub use plot::{SampleDomain, SamplePoint, SampledCurve, SamplingPolicy, sample_1d};
pub use polynomial::{
    CanonicalPolynomial, CoeffRing, CoeffRingTable, CoefficientDomain, CoefficientRingDescriptor, DivisionPolicy, FpBigKernel,
    FINGERPRINT_ALGORITHM, FpWordKernel, GroebnerAlgorithm, GroebnerBasis, GroebnerBasisValue, GroebnerCertificate, GroebnerComputation,
    GroebnerFrontier, GroebnerLimits, GroebnerStatus, GroebnerVerificationReport, Ideal, JitParityOutcome, MonomialLayout,
    MonomialOrder,
    MonomialTerm, Polynomial, PolynomialBuilder, PolynomialCacheKey, PolynomialCacheOp, PolynomialDomainValue,
    PolynomialFingerprint, PolynomialRepr, PolynomialReprBody, PolynomialRequest, PolynomialResult, PolynomialValue, QCoeffKernel,
    ReprTarget, RingCharacteristic, RingDescriptor, RingFingerprint, RingHandle, RingId, RingTable, SpecializedCoeffKernel,
    UnivariateDivision, UnivariateDivisionValue, VerifiedGroebnerBasis, ZCoeffKernel,
    add_polynomial, cache_key_for_request, canonicalize_polynomial, compute_elimination_basis, compute_groebner_basis,
    execute_polynomial, execute_polynomial_mgraph, execute_polynomial_with_rings, ideal_membership, mul_polynomial,
    mul_with_jit_parity, parity_diagnostic, polynomial_canonical_hash, polynomial_fingerprint, polynomial_fingerprint_u64,
    record_polynomial_result, reduce_by_verified, reduce_ideal, reprs_mathematically_equal, resultant_univariate,
    sub_polynomial, div_univariate, gcd_univariate,
    verify_groebner_basis,
};
pub use session::Session;
pub use solver::{
    DomainRef, ReflectionResult, Reflector, SolverContext, SolverLimits, SolverMetadata, SolverOperation, SolverRegistry,
    SolverRequest, score_candidate,
};
pub use term::{Atom, Term, number_from_term};
