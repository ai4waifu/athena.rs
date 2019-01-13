//! M-Graph：面向数学语义的事实图与闭包引擎。
//!
//! M-Graph 建立在 [`athena_ir`] 的 AthenaIR（理论别称 MSM）之上。图中的节点
//! 表示表达式、代数对象或域，边不是无类型依赖，而是带有命题、作用域、保证级别、
//! 证据和依赖的 typed claim。这样可以严格区分“计算得到一个候选值”和“证明了一个
//! 可用于重写的精确事实”。只有 verifier 接受的无条件 `ProvenExact` 等式，才允许
//! 合并到 exact union-find 或驱动语义重写。
//!
//! ## 伽罗瓦连接
//!
//! M-Graph 背后的抽象解释思想，是在具体语义域 `C` 与抽象事实域 `A` 之间维护一对
//! 伴随映射 `α`（抽象）和 `γ`（具体化）：
//!
//! ```text
//! α(c) ⊑ a  当且仅当  c ⊑ γ(a)
//! ```
//!
//! `α` 将具体执行状态压缩成可传播的 typed claims，`γ` 表示这些 claims 所允许的
//! 具体状态集合。抽象可以丢失信息，但不能凭空增加结论。等价类、类型/域约束、
//! 区间、shape 约束和数论证书可以看作不同的抽象域，并通过 hyper-edge 组合。
//!
//! 连接本身不是正确性许可证。solver 可以先产生 tentative、probable 或
//! resource-limited claim，admission gate 必须要求相应的 evidence verifier 通过后，
//! 才能进入 verified fact set。假设依赖和近似保证始终保留在 claim 的 scope 与
//! guarantee 中，不能降级成 unconditional exact。
//!
//! ## 与 KernelIR 的关系
//!
//! ```text
//! AthenaIR / 运行时状态
//!     -- α：抽取 claim --> M-Graph 事实 + 证据
//!     -- 已验证闭包 --> 已验证子图
//!     -- 抽取 --> KernelIR → 守卫 → JIT / eager 回退
//! ```
//!
//! 只有 verified subgraph 才能生成 [`KernelIR`](https://docs.rs/athena-engine) 执行计划。
//! frontier、缓存和预算状态是 operational state，不是数学事实；JIT 失败时必须回退
//! 到 eager 路径而不改变 exact、promotion、rounding 或诊断语义。

mod admission;
mod candidate;
mod claim;
mod closure;
mod congruence;
mod core;
mod derived;
mod exact_uf;
mod fact_log;
mod kernel_ir;
mod operational;
mod polynomial;
mod refs;
mod relation_index;
mod result_cache;
mod scope_index;
mod semantic;
mod state;
mod theory;
mod types;

pub use admission::{
    AdmissionGate, AdmissionOutcome, AdmissionRejectReason, EvidenceVerifier, VerificationPolicy, admit_polynomial_exact,
    admit_polynomial_result, is_admitted,
};
pub use candidate::OuterCandidate;
pub use claim::{Claim, Evidence, Guarantee, Proposition, Scope, VerifiedClaim, proposition_from_cache_key};
pub use closure::{ClosureLimits, ClosureResult, run_closure_step};
pub use core::{ClosureSeeds, MGraphCore, MGraphView};
pub use derived::DerivedIndexes;
pub use exact_uf::ExactUnionFind;
pub use fact_log::{FactId, FactLog};
pub use operational::OperationalState;
pub use polynomial::{
    POLYNOMIAL_SOLVER_ID, PolynomialCacheEntry, PolynomialCacheTier, PolynomialMGraphStore, PolynomialWitness,
    witness_from_exact,
};
pub use refs::{
    RelationRef, RelationStatus, ScopeRef, ScopeRelationKind, WitnessRef, scope_from_ref, scope_ref_from_assumption_set,
    scope_to_ref,
};
pub use relation_index::{RelationIndex, RelationRecord};
pub use result_cache::ResultCache;
pub use scope_index::{ScopeEdge, ScopeIndex};
pub use semantic::SemanticCore;
pub use state::MGraphState;
pub use types::{
    DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, ExactnessLevel, HyperEdge, RewriteWitness,
    SolverCandidate, SolverFrontier, SolverId, SolverScore,
};
