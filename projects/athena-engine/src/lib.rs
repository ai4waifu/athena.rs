//! Athena CAS 执行引擎 — 唯一决定「怎么算」的地方。
//!
//! ```text
//! athena-types → athena-numeric → athena-ir → athena-rewriter → athena-engine → athena
//! ```
//!
//! 本 crate 拥有求值、Session、M-Graph、solver、改写编排、域分派与 `ATHENA_*` 诊断。
//! 不解析方言、不渲染字符串、也不绑定 N-API/WASM。

#![deny(missing_docs)]

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

pub use athena_ir::{AtomKind, SymbolTable, TermArena, TermBuilder, TermKind, canonical_hash};
pub use athena_numeric::{
    ExactInteger, Integer, ModularValue, Modulus, Number, NumericBackend, NumericBackendContract, NumericBackendLimits,
    NumericCapability, NumericDomain, NumericOperation, NumericResultMode, NumericValue, PureRustBackend, number_from_wire,
};
pub use athena_rewriter::{RewriteOptions, RewriteResult, Rewriter};
pub use athena_types::{
    AssumptionSet, AssumptionSetId, Condition, Diagnostic, DiagnosticCode, DiagnosticPath, DiagnosticValue, DomainId,
    ExtensionId, FieldId, GroupElementId, GroupId, NodeId, OperatorId, Precision, Predicate, Result, RoundingMode,
    SerializationVersion, Severity, SourceSpan, SymbolId, TermId,
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
pub use field::{Field, FieldDomainValue, FieldElement, FieldKind, FieldRequest, FieldResult, execute_field};
pub use function::{BranchPolicy, FunctionDefinition, lookup_function, registered_function_names};
pub use galois::{Automorphism, GaloisDomainValue, GaloisGroup, GaloisRequest, GaloisResult, execute_galois};
pub use group::{
    Group, GroupDomainValue, GroupElement, GroupElementRepr, GroupKind, GroupRequest, GroupResult, Permutation, execute_group,
};
pub use mgraph::{
    ClosureLimits, ClosureResult, DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, ExactnessLevel,
    HyperEdge, MGraphState, RewriteWitness, SolverCandidate, SolverFrontier, SolverId, SolverScore, run_closure_step,
};
pub use number_theory::{
    ExtendedGcd, FactorLimits, Factorization, FactorizationCompleteness, NumberTheoryRequest, NumberTheoryResult,
    NumberTheoryValue, Primality, PrimePower, execute_number_theory, extended_gcd, factor_integer, gcd, lcm, mod_inverse,
    mod_pow, primality_test,
};
pub use plot::{SampleDomain, SamplePoint, SampledCurve, SamplingPolicy, sample_1d};
pub use polynomial::{
    CanonicalPolynomial, CoefficientDomain, DivisionPolicy, GroebnerAlgorithm, GroebnerBasis, GroebnerBasisValue,
    GroebnerCertificate, GroebnerLimits, Ideal, MonomialOrder, MonomialTerm, Polynomial, PolynomialBuilder,
    PolynomialDomainValue, PolynomialRepr, PolynomialReprBody, PolynomialRequest, PolynomialResult, PolynomialValue,
    ReprTarget, RingCharacteristic, RingDescriptor, RingId, RingTable, add_polynomial, canonicalize_polynomial,
    compute_elimination_basis, compute_groebner_basis, execute_polynomial, execute_polynomial_with_rings, mul_polynomial,
    polynomial_canonical_hash, reduce_ideal, reprs_mathematically_equal, sub_polynomial,
};
pub use session::Session;
pub use solver::{
    DomainRef, ReflectionResult, Reflector, SolverContext, SolverLimits, SolverMetadata, SolverOperation, SolverRegistry,
    SolverRequest, score_candidate,
};
pub use term::{Atom, Term, number_from_term};
