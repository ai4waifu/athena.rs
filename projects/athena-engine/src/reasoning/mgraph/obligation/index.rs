//! 义务索引与 Reflector 唤醒队列（· bootstrap）。
//!
//! 挂起的 [`ProofObligation`] 住在运行态。接纳可唤醒匹配义务；唤醒本身 **不**写入 SemanticCore。

use crate::reasoning::mgraph::{
    core::refs::{PredicateId, RelationRef, ScopeRef},
    obligation::ProofObligation,
    relations::scope::ScopeIndex,
};

/// 一次接纳后产生的 Reflector 唤醒。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct ReflectorWake {
    /// 现在可以再次反射的义务。
    pub obligation: ProofObligation,
    /// 匹配到的新接纳关系。
    pub relation: RelationRef,
}

impl ReflectorWake {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { obligation: self.obligation.owning_copy(), relation: self.relation }
    }
}

/// 单次接纳后排空唤醒的报告。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq, Default)]
pub struct WakeReport {
    /// 已从挂起索引移除并交给调用方的义务。
    pub wakes: Vec<ReflectorWake>,
}

impl WakeReport {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { wakes: self.wakes.iter().map(ReflectorWake::owning_copy).collect() }
    }
}

/// 按谓词 / 作用域匹配键索引的挂起义务。
///
/// **不**实现 [`Clone`]。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ObligationIndex {
    pending: Vec<ProofObligation>,
}

impl ObligationIndex {
    /// 空索引。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一条挂起的语义缺口。
    pub fn register(&mut self, obligation: ProofObligation) {
        self.pending.push(obligation);
    }

    /// 挂起义务条数。
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// 是否没有挂起义务。
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// 唤醒并移除可在 `admitted_scope` 上观察 `predicate` 的义务。
    ///
    /// 匹配条件：谓词相同、作用域非 `IncompatibleWith`，且义务能经
    /// 恒等 / `Refines` 祖先 / 有向 `CompatibleWith` 看见被接纳的纤维。
    pub fn wake_matching(
        &mut self,
        admitted_scope: ScopeRef,
        predicate: PredicateId,
        relation: RelationRef,
        scopes: &ScopeIndex,
    ) -> WakeReport {
        let mut wakes = Vec::new();
        let mut retained = Vec::new();
        for obligation in self.pending.drain(..) {
            let visible = obligation.predicate == predicate
                && !scopes.incompatible_with(obligation.scope, admitted_scope)
                && (scopes.is_refines_ancestor(obligation.scope, admitted_scope) || scopes.compatible_with(obligation.scope, admitted_scope));
            if visible {
                wakes.push(ReflectorWake { obligation, relation });
            }
            else {
                retained.push(obligation);
            }
        }
        self.pending = retained;
        WakeReport { wakes }
    }
}
