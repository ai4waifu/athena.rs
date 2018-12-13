//! Scope 索引：scope 间最小关系（**非** world registry / **非** rooted tree）。
//! 理论层 `Σ`（ScopeRelation）→ 实现层 `ScopeIndex`。见 [`super::theory`]。

use super::refs::{ScopeRef, ScopeRelationKind};

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
}
