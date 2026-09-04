//! 实现层 M-Graph 语义核心。
//!
//! 理论层完整规格见 [`crate::reasoning::mgraph::relations::theory`]（**非运行时**）。
//! 实现层：`MGraphCore { scope_index, relation_index }` + `admit` / `close`。
//! 理论层概念（根源宇宙、纤维化、内世界子范畴）**不得**出现在本模块的类型中。

pub mod refs;
pub mod state;
pub mod types;

pub use refs::{
    PredicateId, RelationRef, RelationStatus, ScopeRef, ScopeRelationKind, SemanticRef, WitnessRef, predicates,
    scope_from_ref, scope_ref_from_assumption_set, scope_to_ref,
};
pub use state::MGraphState;
pub use types::{
    DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, ExactnessLevel, HyperEdge, RewriteWitness,
    SolverCandidate, SolverFrontier, SolverId, SolverScore,
};

use crate::reasoning::mgraph::{
    facts::claim::VerifiedClaim,
    relations::{
        index::{RelationIndex, RelationRecord},
        scope::ScopeIndex,
    },
};
/// 闭包传播种子（按需扩展）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosureSeeds {
    /// 起始 scope（空 = 全图按需）。
    pub scopes: Vec<ScopeRef>,
}

/// 实现层 M-Graph 语义核心（**无** RootUniverse / OuterWorld / 全局对象表）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MGraphCore {
    scope_index: ScopeIndex,
    relation_index: RelationIndex,
}

impl MGraphCore {
    /// 空 core。
    pub fn new() -> Self {
        Self::default()
    }

    /// Scope 关系索引（只读）。
    pub fn scope_index(&self) -> &ScopeIndex {
        &self.scope_index
    }

    /// Scope 关系索引（可变）。
    pub fn scope_index_mut(&mut self) -> &mut ScopeIndex {
        &mut self.scope_index
    }

    /// 关系索引（只读）。
    pub fn relation_index(&self) -> &RelationIndex {
        &self.relation_index
    }

    /// 关系索引（可变）。
    pub fn relation_index_mut(&mut self) -> &mut RelationIndex {
        &mut self.relation_index
    }

    /// 仅由 [`crate::reasoning::mgraph::admission::semantic::SemanticCore`] 调用。
    ///
    /// 公开写入必须经 [`crate::reasoning::mgraph::admission::gate::AdmissionGate`]。
    pub(crate) fn admit(&mut self, claim: VerifiedClaim) -> RelationRef {
        let record = RelationRecord::from_verified(claim);
        self.relation_index.append(record)
    }

    /// 显式注册 scope 细化边（transport 规则的一部分）。
    pub fn refine_scope(&mut self, from: ScopeRef, to: ScopeRef) {
        self.scope_index.add_relation(from, to, ScopeRelationKind::Refines);
    }

    /// 对已接纳关系做必要闭包传播（transport 规则待扩展）。
    pub fn close(&mut self, _seeds: &ClosureSeeds) {
        // Transport / projection 规则待扩展。
    }

    /// 关系条数。
    pub fn relation_count(&self) -> usize {
        self.relation_index.count()
    }
}

/// 只读查询视图（不暴露 operational 状态）。
#[derive(Debug, Clone, Copy)]
pub struct MGraphView<'a> {
    core: &'a MGraphCore,
}

impl<'a> MGraphView<'a> {
    /// 包装 core 引用。
    pub fn new(core: &'a MGraphCore) -> Self {
        Self { core }
    }

    /// 某 scope 下已接纳关系 id。
    pub fn relations_in_scope(&self, scope: ScopeRef) -> &[RelationRef] {
        self.core.relation_index().relations_in_scope(scope)
    }

    /// 按 id 查关系记录。
    pub fn relation(&self, id: RelationRef) -> Option<&RelationRecord> {
        self.core.relation_index().get(id)
    }

    /// Scope 边。
    pub fn scope_edges(&self) -> &[crate::reasoning::mgraph::relations::scope::ScopeEdge] {
        self.core.scope_index().edges()
    }
}
