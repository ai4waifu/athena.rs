//! Scope 索引：scope 间最小关系（**非** world registry / **非** rooted tree）。
//! 理论层 `Σ`（ScopeRelation）→ 实现层 `ScopeIndex`。见 [`crate::reasoning::mgraph::relations::theory`]。

use athena_types::{Diagnostic, DiagnosticCode};

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

/// Scope 关系注册冲突（transport / merge 诊断 · Living `29`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeRelationConflict {
    /// 同一对 scope 上同时要求 Compatible 与 Incompatible。
    CompatibleAndIncompatible {
        /// 一端。
        a: ScopeRef,
        /// 另一端。
        b: ScopeRef,
    },
    /// 注册 `from ⊑ to` 会与已有 `Refines` 形成环。
    RefinesWouldCycle {
        /// 拟细化源。
        from: ScopeRef,
        /// 拟细化目标。
        to: ScopeRef,
    },
    /// Scope 与自身 Incompatible（无意义，禁止）。
    SelfIncompatible {
        /// 冲突 scope。
        scope: ScopeRef,
    },
}

impl ScopeRelationConflict {
    /// 机器可读 reason 键。
    pub fn reason_key(&self) -> &'static str {
        match self {
            Self::CompatibleAndIncompatible { .. } => "compatible_and_incompatible",
            Self::RefinesWouldCycle { .. } => "refines_would_cycle",
            Self::SelfIncompatible { .. } => "self_incompatible",
        }
    }

    /// 转为结构化诊断（禁止静默吞掉 merge 冲突）。
    pub fn into_diagnostic(self) -> Diagnostic {
        let reason = self.reason_key();
        let mut diag = Diagnostic::new(DiagnosticCode::AssumptionUnresolved)
            .detail("domain", "mgraph")
            .detail("operation", "scope_relation")
            .detail("reason", reason);
        match self {
            Self::CompatibleAndIncompatible { a, b } => {
                diag = diag.detail("scope_a", a.0.to_string()).detail("scope_b", b.0.to_string());
            }
            Self::RefinesWouldCycle { from, to } => {
                diag = diag.detail("from", from.0.to_string()).detail("to", to.0.to_string());
            }
            Self::SelfIncompatible { scope } => {
                diag = diag.detail("scope", scope.0.to_string());
            }
        }
        diag
    }
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
    ///
    /// 冲突时返回 [`ScopeRelationConflict`] 且**不**写入边。
    pub fn try_add_relation(
        &mut self,
        from: ScopeRef,
        to: ScopeRef,
        kind: ScopeRelationKind,
    ) -> Result<(), ScopeRelationConflict> {
        if self.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind) {
            return Ok(());
        }
        match kind {
            ScopeRelationKind::IncompatibleWith => {
                if from == to {
                    return Err(ScopeRelationConflict::SelfIncompatible { scope: from });
                }
                if self.compatible_with(from, to) || self.compatible_with(to, from) {
                    return Err(ScopeRelationConflict::CompatibleAndIncompatible { a: from, b: to });
                }
            }
            ScopeRelationKind::CompatibleWith => {
                if self.incompatible_with(from, to) {
                    return Err(ScopeRelationConflict::CompatibleAndIncompatible { a: from, b: to });
                }
            }
            ScopeRelationKind::Refines => {
                // `from ⊑ to` while `to ⊑* from` would close a cycle.
                if from != to && self.is_refines_ancestor(to, from) {
                    return Err(ScopeRelationConflict::RefinesWouldCycle { from, to });
                }
            }
            ScopeRelationKind::Restricts => {}
        }
        self.edges.push(ScopeEdge { from, to, kind });
        Ok(())
    }

    /// 注册 scope 关系（冲突时 panic · 仅用于已证明无冲突的内部路径）。
    ///
    /// Prefer [`Self::try_add_relation`] at public / planner boundaries.
    pub fn add_relation(&mut self, from: ScopeRef, to: ScopeRef, kind: ScopeRelationKind) {
        self.try_add_relation(from, to, kind).unwrap_or_else(|conflict| {
            panic!("scope relation conflict: {}", conflict.reason_key());
        });
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
        self.edges.iter().filter(move |e| e.from == from && e.kind == ScopeRelationKind::Refines).map(|e| e.to)
    }

    /// Whether an undirected `IncompatibleWith` edge links `a` and `b`.
    pub fn incompatible_with(&self, a: ScopeRef, b: ScopeRef) -> bool {
        self.edges.iter().any(|e| e.kind == ScopeRelationKind::IncompatibleWith && ((e.from == a && e.to == b) || (e.from == b && e.to == a)))
    }

    /// Whether a directed `CompatibleWith` edge `from → to` exists.
    pub fn compatible_with(&self, from: ScopeRef, to: ScopeRef) -> bool {
        self.edges.iter().any(|e| e.from == from && e.to == to && e.kind == ScopeRelationKind::CompatibleWith)
    }

    /// Direct `CompatibleWith` peers of `from` (`from` may consult `to` locally).
    pub fn compatible_peers(&self, from: ScopeRef) -> impl Iterator<Item = ScopeRef> + '_ {
        self.edges.iter().filter(move |e| e.from == from && e.kind == ScopeRelationKind::CompatibleWith).map(|e| e.to)
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

#[cfg(test)]
mod tests {
    use super::{ScopeIndex, ScopeRelationConflict};
    use crate::reasoning::mgraph::core::refs::{ScopeRef, ScopeRelationKind};

    #[test]
    fn rejects_compatible_then_incompatible() {
        let mut scopes = ScopeIndex::new();
        let a = ScopeRef(1);
        let b = ScopeRef(2);
        scopes.try_add_relation(a, b, ScopeRelationKind::CompatibleWith).expect("compatible");
        let err = scopes.try_add_relation(a, b, ScopeRelationKind::IncompatibleWith).expect_err("conflict");
        assert_eq!(err, ScopeRelationConflict::CompatibleAndIncompatible { a, b });
        assert!(!scopes.incompatible_with(a, b));
        assert_eq!(err.into_diagnostic().details.get("reason").map(|v| v.to_string()).as_deref(), Some("compatible_and_incompatible"));
    }

    #[test]
    fn rejects_incompatible_then_compatible() {
        let mut scopes = ScopeIndex::new();
        let a = ScopeRef(3);
        let b = ScopeRef(4);
        scopes.try_add_relation(a, b, ScopeRelationKind::IncompatibleWith).expect("incompatible");
        let err = scopes.try_add_relation(a, b, ScopeRelationKind::CompatibleWith).expect_err("conflict");
        assert!(matches!(err, ScopeRelationConflict::CompatibleAndIncompatible { .. }));
        assert!(!scopes.compatible_with(a, b));
    }

    #[test]
    fn rejects_refines_cycle() {
        let mut scopes = ScopeIndex::new();
        let a = ScopeRef(5);
        let b = ScopeRef(6);
        scopes.try_add_relation(a, b, ScopeRelationKind::Refines).expect("a ⊑ b");
        let err = scopes.try_add_relation(b, a, ScopeRelationKind::Refines).expect_err("cycle");
        assert_eq!(err, ScopeRelationConflict::RefinesWouldCycle { from: b, to: a });
    }

    #[test]
    fn rejects_self_incompatible() {
        let s = ScopeRef(7);
        let mut scopes = ScopeIndex::new();
        let err = scopes.try_add_relation(s, s, ScopeRelationKind::IncompatibleWith).expect_err("self");
        assert_eq!(err, ScopeRelationConflict::SelfIncompatible { scope: s });
    }
}
