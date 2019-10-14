//! M-Graph：面向数学语义的事实图与闭包引擎。
//!
//! 子树职责：
//! - [`admission`] — 证据门控与候选
//! - [`facts`] — claim / admission journal
//! - [`equivalence`] — exact UF / 同余
//! - [`relations`] — scope / relation 索引与理论规格
//! - [`closure`] — 闭包步进与 operational state
//! - [`cache`] — 结果缓存
//! - [`core`] — 实现层核心类型与状态
//! - [`obligation`] — ProofObligation / SemanticReflector（Living `29`）
//! - [`semantic_entry`] — Living `29` 顶层 Goal → Reflector → Plan（非裸 `execute_domain`）
//! - [`polynomial`] — 多项式域 M-Graph 存储 / witness

pub mod admission;
pub mod cache;
pub mod closure;
pub mod core;
pub mod equivalence;
pub mod facts;
pub mod obligation;
pub mod polynomial;
pub mod reflectors;
pub mod relations;
pub mod semantic_entry;

pub use admission::{
    AdmissionGate, AdmissionOutcome, AdmissionRejectReason, EvidenceVerifier, OuterCandidate, SemanticCore, VerificationPolicy,
    admit_polynomial_exact, admit_polynomial_result, is_admitted,
};
pub use cache::ResultCache;
pub use closure::{ClosureLimits, ClosureResult, OperationalState, run_closure_step};
pub use core::{
    CapabilityProviderId, ClosureSeeds, DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, ExactnessLevel, HyperEdge,
    MGraphCore, MGraphState, MGraphView, ObjectRef, PredicateId, RelationRef, RelationStatus, RewriteWitness, ScopeRef, ScopeRelationKind,
    SemanticRef, SolverCandidate, SolverFrontier, SolverScore, TheoryContextId, WitnessRef, predicates, scope_from_ref,
    scope_ref_from_assumption_set, scope_to_ref,
};
pub use equivalence::ExactUnionFind;
pub use facts::{
    AdmissionJournal, Claim, Evidence, EvidenceCertificate, FactId, Guarantee, Proposition, Scope, VerifiedClaim, proposition_from_cache_key,
};
pub use obligation::{ProofObligation, Reflection, SemanticReflector};
pub use polynomial::{
    POLYNOMIAL_PROVIDER_ID, PolynomialCacheEntry, PolynomialCacheTier, PolynomialMGraphStore, PolynomialWitness, witness_from_exact,
};
pub use reflectors::{CalculusReflector, PolynomialReflector};
pub use relations::{DerivedIndexes, RelationIndex, RelationRecord, ScopeEdge, ScopeIndex};
pub use semantic_entry::{DomainSemanticOutcome, execute_domain_goal, obligation_from_domain_request};
