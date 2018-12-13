//! 理论层：`ScopeRef` 实现 `𝓦` 索引；**非** `struct World`。
//! 完整公理与对照表见 [`super::theory`]。

use athena_types::AssumptionSetId;

use super::claim::Scope;

/// 语义作用域引用（实现层 `ScopeRef`，非理论层「内世界」对象）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopeRef(pub u32);

impl ScopeRef {
    /// 无条件作用域（默认 scope）。
    pub const UNCONDITIONAL: Self = Self(0);
}

/// 已接纳关系引用（与 [`super::fact_log::FactId`] 同构，单调递增）。
pub type RelationRef = super::fact_log::FactId;

/// 命题引用（当前与 [`RelationRef`] 同 id 空间；完整命题由领域模块解释）。
pub type PropositionRef = RelationRef;

/// 外部可验证证据引用（详细载荷在 `WitnessStore` 或 claim 内联）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WitnessRef(pub u64);

/// 关系在 scope 内的接纳状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationStatus {
    /// 已验证接纳。
    Accepted,
    /// 条件下接纳。
    Conditional,
    /// 已有反证。
    Refuted,
}

/// Scope 之间的最小关系（非完整 world DAG materialization）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeRelationKind {
    /// `from` 细化/强于 `to`（沿 transport 可向下继承时注册）。
    Refines,
    /// 限制到更窄上下文。
    Restricts,
    /// 可并存。
    CompatibleWith,
    /// 不可并存。
    IncompatibleWith,
}

/// 将 claim 合同中的 [`Scope`] 编码为 [`ScopeRef`]（不分配 registry）。
pub fn scope_to_ref(scope: Scope) -> ScopeRef {
    match scope {
        Scope::Unconditional => ScopeRef::UNCONDITIONAL,
        Scope::UnderAssumptions(id) => scope_ref_from_assumption_set(id),
    }
}

/// 假设集 id → scope 引用（`0` 保留给无条件 scope）。
pub fn scope_ref_from_assumption_set(id: AssumptionSetId) -> ScopeRef {
    ScopeRef(id.0.wrapping_add(1))
}

/// 若可能，还原为 claim [`Scope`]。
pub fn scope_from_ref(scope: ScopeRef) -> Scope {
    if scope == ScopeRef::UNCONDITIONAL {
        Scope::Unconditional
    }
    else {
        Scope::UnderAssumptions(AssumptionSetId(scope.0.wrapping_sub(1)))
    }
}
