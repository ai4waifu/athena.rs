//! 实现层 M-Graph 语义核心。
//!
//! 理论层完整规格见 [`crate::reasoning::mgraph::relations::theory`]（**非运行时**）。
//! 实现层：`MGraphCore { scope_index, relation_index }` + `admit` / `close`。
//! 理论层概念（根源宇宙、纤维化、内世界子范畴）**不得**出现在本模块的类型中。

pub mod predicate_registry;
pub mod refs;
pub mod state;
pub mod types;

pub use predicate_registry::{PredicateDescriptor, all_descriptors, arity_ok, descriptor};
pub use refs::{
    ObjectRef, PredicateId, RelationRef, RelationStatus, ScopeRef, ScopeRelationKind, SemanticRef, TheoryContextId, WitnessRef, predicates,
    scope_from_ref, scope_ref_from_assumption_set, scope_to_ref,
};
pub use state::MGraphState;
pub use types::{
    CapabilityProviderId, DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, ExactnessLevel, HyperEdge,
    RewriteWitness, SolverCandidate, SolverFrontier, SolverScore,
};

use crate::reasoning::mgraph::{
    facts::claim::VerifiedClaim,
    relations::{
        index::{RelationIndex, RelationRecord},
        scope::{ScopeIndex, ScopeRelationConflict},
    },
};
/// 闭包传播种子（按需扩展）。
///
/// Living `31`：**不**实现 [`Clone`]（语义路径容器；优先按值移动 / 重建）。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ClosureSeeds {
    /// 起始 scope（空 = 全图按需）。
    pub scopes: Vec<ScopeRef>,
}

/// 实现层 M-Graph 语义核心（**无** RootUniverse / OuterWorld / 全局对象表）。
///
/// Living `31`：**不**实现 [`Clone`]（含 owning [`RelationIndex`]）。
#[derive(Debug, Default, PartialEq, Eq)]
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

    /// 关系索引（只读）。外部写入必须经 [`crate::reasoning::mgraph::admission::gate::AdmissionGate`]。
    pub fn relation_index(&self) -> &RelationIndex {
        &self.relation_index
    }

    /// 仅由 [`crate::reasoning::mgraph::admission::semantic::SemanticCore`] 调用。
    ///
    /// 公开写入必须经 [`crate::reasoning::mgraph::admission::gate::AdmissionGate`]。
    pub(crate) fn admit(&mut self, claim: VerifiedClaim) -> RelationRef {
        let record = RelationRecord::from_verified(claim);
        self.relation_index.append(record)
    }

    /// 用 journal 重建的查询索引整表替换（保留 scope 边）。
    pub(crate) fn replace_relation_index(&mut self, index: RelationIndex) {
        self.relation_index = index;
    }

    /// 显式注册 scope 细化边（transport 规则的一部分）。
    pub fn refine_scope(&mut self, from: ScopeRef, to: ScopeRef) -> Result<(), ScopeRelationConflict> {
        self.scope_index.try_add_relation(from, to, ScopeRelationKind::Refines)
    }

    /// Register that `from` may consult local facts of `to` (Living `29` Compatible).
    pub fn mark_scopes_compatible(&mut self, from: ScopeRef, to: ScopeRef) -> Result<(), ScopeRelationConflict> {
        self.scope_index.try_add_relation(from, to, ScopeRelationKind::CompatibleWith)
    }

    /// Register that `a` and `b` must not share query-time transport (Living `29` Incompatible).
    pub fn mark_scopes_incompatible(&mut self, a: ScopeRef, b: ScopeRef) -> Result<(), ScopeRelationConflict> {
        self.scope_index.try_add_relation(a, b, ScopeRelationKind::IncompatibleWith)
    }

    /// 对已接纳关系做必要闭包传播（当前：经 [`crate::reasoning::mgraph::run_closure_step`] 物化传递性证明边）。
    ///
    /// `seeds` 预留 scope 过滤；bootstrap 忽略并在全 semantic 上运行。
    /// Scope `Refines` transport remains **query-time** via [`MGraphView::find_accepted`]
    /// (no fiber copying into unconditional closure).
    pub fn close(&mut self, _seeds: &ClosureSeeds) {
        let _ = self;
        // Equality-forest closure runs on [`MGraphState`] via [`run_closure_step`].
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

    /// 在 `scope` 中查找已接纳 / 条件下接纳的谓词命中（Living `29` Reflector 短路）。
    ///
    /// Query-time transport: also search scopes reachable via registered
    /// [`ScopeRelationKind::Refines`] edges (`scope ⊑ ancestor`), plus local
    /// [`ScopeRelationKind::CompatibleWith`] peers. Fibers marked
    /// [`ScopeRelationKind::IncompatibleWith`] the query scope are skipped.
    /// Does **not** copy relations into other fibers or the unconditional closure.
    pub fn find_accepted_by_predicate(&self, scope: ScopeRef, predicate: PredicateId) -> Option<RelationRef> {
        self.find_accepted(scope, predicate, &[])
    }

    /// 按谓词与已知对象前缀匹配已接纳关系（对象须按 subject 中 `Object` 顺序对齐）。
    ///
    /// Transport rules (bootstrap):
    /// - Walk `Refines` ancestors (`scope ⊑* ancestor`), skipping fibers marked
    ///   `IncompatibleWith` the query scope.
    /// - Additionally consult **local** facts of `CompatibleWith` peers (no peer
    ///   ancestor expansion). `IncompatibleWith` wins over `CompatibleWith`.
    pub fn find_accepted(&self, scope: ScopeRef, predicate: PredicateId, known_objects: &[ObjectRef]) -> Option<RelationRef> {
        let scopes = self.core.scope_index();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![scope];
        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }
            if scopes.incompatible_with(scope, current) {
                continue;
            }
            if let Some(id) = self.find_accepted_local(current, predicate, known_objects) {
                return Some(id);
            }
            for ancestor in scopes.refines_targets(current) {
                if !scopes.incompatible_with(scope, ancestor) {
                    stack.push(ancestor);
                }
            }
            // Compatible peers: local fiber only (do not push onto Refines walk).
            for peer in scopes.compatible_peers(current) {
                if visited.contains(&peer) || scopes.incompatible_with(scope, peer) {
                    continue;
                }
                if let Some(id) = self.find_accepted_local(peer, predicate, known_objects) {
                    return Some(id);
                }
                let _ = visited.insert(peer);
            }
        }
        None
    }

    fn find_accepted_local(&self, scope: ScopeRef, predicate: PredicateId, known_objects: &[ObjectRef]) -> Option<RelationRef> {
        self.core.relation_index().relations_with_predicate(scope, predicate).iter().copied().find(|&id| {
            self.relation(id).is_some_and(|r| {
                if !matches!(r.status, RelationStatus::Accepted | RelationStatus::Conditional) {
                    return false;
                }
                if known_objects.is_empty() {
                    return true;
                }
                let object_subjects: Vec<ObjectRef> = r
                    .subjects
                    .iter()
                    .filter_map(|s| match s {
                        SemanticRef::Object(o) => Some(*o),
                        _ => None,
                    })
                    .collect();
                known_objects.len() <= object_subjects.len() && known_objects.iter().zip(object_subjects.iter()).all(|(want, got)| want == got)
            })
        })
    }

    /// Scope 边。
    pub fn scope_edges(&self) -> &[crate::reasoning::mgraph::relations::scope::ScopeEdge] {
        self.core.scope_index().edges()
    }
}
