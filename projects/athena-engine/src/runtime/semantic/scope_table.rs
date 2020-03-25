//! [`AssumptionScope`] Session intern 表。

use std::collections::BTreeMap;

use athena_types::{AssumptionScope, AssumptionScopeId, Diagnostic, DiagnosticCode, Predicate, SymbolId};

/// 假设作用域 intern 表。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssumptionScopeTable {
    scopes: BTreeMap<AssumptionScopeId, AssumptionScope>,
    next: u32,
}

impl AssumptionScopeTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 写入作用域并分配 [`AssumptionScopeId`]（回写 `scope.id`）。
    pub fn intern(&mut self, mut scope: AssumptionScope) -> Result<AssumptionScopeId, Diagnostic> {
        if let Some(conflict) = scope.local_conflict() {
            return Err(Diagnostic::new(DiagnosticCode::TypeMismatch)
                .detail("domain", "assumption_scope")
                .detail("operation", "intern")
                .detail("reason", "local_conflict")
                .detail("conflict_kind", format!("{:?}", conflict.kind)));
        }
        if let Some(parent) = scope.parent {
            if !self.scopes.contains_key(&parent) {
                return Err(Diagnostic::new(DiagnosticCode::TypeMismatch)
                    .detail("domain", "assumption_scope")
                    .detail("operation", "intern")
                    .detail("reason", "missing_parent")
                    .arg("parent", parent.0));
            }
        }
        let id = AssumptionScopeId(self.next);
        self.next = self.next.saturating_add(1);
        scope.id = Some(id);
        self.scopes.insert(id, scope);
        Ok(id)
    }

    /// 查找。
    pub fn get(&self, id: AssumptionScopeId) -> Option<&AssumptionScope> {
        self.scopes.get(&id)
    }

    /// 展开继承谓词（根 → 叶）。
    pub fn inherited_predicates(&self, id: AssumptionScopeId) -> Result<Vec<Predicate>, Diagnostic> {
        let scope = self.get(id).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::TypeMismatch)
                .detail("domain", "assumption_scope")
                .detail("operation", "inherited_predicates")
                .detail("reason", "missing_scope")
                .arg("scope_id", id.0)
        })?;
        Ok(scope.inherited_predicates(|pid| self.get(pid).cloned()))
    }

    /// 合并两个已 intern 作用域，成功则 intern 结果。
    pub fn merge_interned(&mut self, left: AssumptionScopeId, right: AssumptionScopeId) -> Result<AssumptionScopeId, Diagnostic> {
        let a = self.get(left).cloned().ok_or_else(|| missing(left))?;
        let b = self.get(right).cloned().ok_or_else(|| missing(right))?;
        match a.merge(&b) {
            athena_types::ScopeMergeOutcome::Ok(merged) => self.intern(merged),
            athena_types::ScopeMergeOutcome::Conflict(c) => Err(Diagnostic::new(DiagnosticCode::TypeMismatch)
                .detail("domain", "assumption_scope")
                .detail("operation", "merge_interned")
                .detail("reason", "merge_conflict")
                .detail("conflict_kind", format!("{:?}", c.kind))),
        }
    }

    /// 投影：展开继承谓词后，仅保留与给定符号相关的符号级谓词（assumption projection bootstrap）。
    ///
    /// 返回**未 intern** 的扁平作用域（无 parent · 无 id）。项级谓词在符号未知时保守丢弃。
    pub fn project_to_symbols(&self, id: AssumptionScopeId, symbols: &[SymbolId]) -> Result<AssumptionScope, Diagnostic> {
        let scope = self.get(id).ok_or_else(|| missing(id))?;
        let predicates = self.inherited_predicates(id)?;
        let flat = AssumptionScope {
            id: None,
            parent: None,
            predicates,
            theory_context: scope.theory_context.clone(),
            branch_policy: scope.branch_policy,
            coefficient_domain: scope.coefficient_domain,
            precision_policy: scope.precision_policy,
        };
        Ok(flat.project_to_symbols(symbols))
    }

    /// 投影并 intern 结果。
    pub fn project_interned(&mut self, id: AssumptionScopeId, symbols: &[SymbolId]) -> Result<AssumptionScopeId, Diagnostic> {
        let projected = self.project_to_symbols(id, symbols)?;
        self.intern(projected)
    }

    /// 已登记数量。
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }
}

fn missing(id: AssumptionScopeId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::TypeMismatch)
        .detail("domain", "assumption_scope")
        .detail("operation", "lookup")
        .detail("reason", "missing_scope")
        .arg("scope_id", id.0)
}
