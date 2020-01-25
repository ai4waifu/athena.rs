//! Scope 索引：scope 间最小关系（**非** world registry / **非** rooted tree）。
//! 理论层 `Σ`（ScopeRelation）→ 实现层 `ScopeIndex`。见 [`crate::reasoning::mgraph::relations::theory`]。

use crate::reasoning::mgraph::core::refs::{ScopeRef, ScopeRelationKind};

/// Scope 间已注册关系边。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeEdge {
    /// 源 scope。
    pub from: ScopeRef,
    /// 目标 scope。
    pub to: ScopeRef,
    /// 关系种类。
    pub kind: ScopeRelationKind,
}

/// 实现层 scope 关系索引（仅保存边，不物化完整范畴）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeIndex {
    edges: Vec<ScopeEdge>,
}

impl ScopeIndex {
    /// 空索引。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册 scope 关系（显式 transport 规则的一部分）。
    pub fn add_relation(&mut self, from: ScopeRef, to: ScopeRef, kind: ScopeRelationKind) {
        self.edges.push(ScopeEdge { from, to, kind });
    }

    /// 全部 scope 边（只读）。
    pub fn edges(&self) -> &[ScopeEdge] {
        &self.edges
    }

    /// 是否存在 `from` refines `to` 的边。
    pub fn refines(&self, from: ScopeRef, to: ScopeRef) -> bool {
        self.edges.iter().any(|e| e.from == from && e.to == to && e.kind == ScopeRelationKind::Refines)
    }

    /// Direct `Refines` targets of `from` (`from ⊑ to`).
    pub fn refines_targets(&self, from: ScopeRef) -> impl Iterator<Item = ScopeRef> + '_ {
        self.edges
            .iter()
            .filter(move |e| e.from == from && e.kind == ScopeRelationKind::Refines)
            .map(|e| e.to)
    }

    /// Whether an undirected `IncompatibleWith` edge links `a` and `b`.
    pub fn incompatible_with(&self, a: ScopeRef, b: ScopeRef) -> bool {
        self.edges.iter().any(|e| {
            e.kind == ScopeRelationKind::IncompatibleWith
                && ((e.from == a && e.to == b) || (e.from == b && e.to == a))
        })
    }

    /// Whether a directed `CompatibleWith` edge `from → to` exists.
    pub fn compatible_with(&self, from: ScopeRef, to: ScopeRef) -> bool {
        self.edges
            .iter()
            .any(|e| e.from == from && e.to == to && e.kind == ScopeRelationKind::CompatibleWith)
    }

    /// Direct `CompatibleWith` peers of `from` (`from` may consult `to` locally).
    pub fn compatible_peers(&self, from: ScopeRef) -> impl Iterator<Item = ScopeRef> + '_ {
        self.edges
            .iter()
            .filter(move |e| e.from == from && e.kind == ScopeRelationKind::CompatibleWith)
            .map(|e| e.to)
    }

    /// Whether `from` reaches `ancestor` by zero or more `Refines` steps (`from ⊑* ancestor`).
    pub fn is_refines_ancestor(&self, from: ScopeRef, ancestor: ScopeRef) -> bool {
        if from == ancestor {
            return true;
        }
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![from];
        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }
            for to in self.refines_targets(current) {
                if to == ancestor {
                    return true;
                }
                stack.push(to);
            }
        }
        false
    }
}
