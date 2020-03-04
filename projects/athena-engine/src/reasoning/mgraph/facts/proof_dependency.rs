//! Proof dependency index：已接纳事实对前提事实的可重放依赖（Living `29`）。
//!
//! 禁止用 cache 命中 / 耗时冒充证明依赖。依赖边只记录 [`FactId`]，不复制 claim 载荷。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use athena_types::{Diagnostic, DiagnosticCode};

use super::journal::FactId;

/// `FactId` → 前提 `FactId` 列表（append-only 登记 · 可查询）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofDependencyIndex {
    deps: BTreeMap<FactId, Vec<FactId>>,
}

impl ProofDependencyIndex {
    /// 空索引。
    pub fn new() -> Self {
        Self::default()
    }

    /// 为已接纳事实登记前提依赖。
    ///
    /// - 禁止自依赖
    /// - 前提 id 必须严格小于当前事实（journal 单调序 · bootstrap）
    /// - 重复登记同一 `fact` 返回冲突诊断（不覆盖）
    pub fn record(&mut self, fact: FactId, premises: &[FactId]) -> Result<(), Diagnostic> {
        if self.deps.contains_key(&fact) {
            return Err(diag("duplicate_dependency_record").detail("fact", fact.0.to_string()));
        }
        let mut unique = BTreeSet::new();
        for premise in premises {
            if *premise == fact {
                return Err(diag("self_dependency").detail("fact", fact.0.to_string()));
            }
            if premise.0 >= fact.0 {
                return Err(diag("premise_not_prior")
                    .detail("fact", fact.0.to_string())
                    .detail("premise", premise.0.to_string()));
            }
            unique.insert(*premise);
        }
        self.deps.insert(fact, unique.into_iter().collect());
        Ok(())
    }

    /// 直接前提（无传递）。
    pub fn premises(&self, fact: FactId) -> &[FactId] {
        self.deps.get(&fact).map(Vec::as_slice).unwrap_or(&[])
    }

    /// 是否登记过依赖（空前提列表也算已登记）。
    pub fn is_recorded(&self, fact: FactId) -> bool {
        self.deps.contains_key(&fact)
    }

    /// `fact` 是否（传递地）依赖 `premise`。
    pub fn depends_on(&self, fact: FactId, premise: FactId) -> bool {
        if fact == premise {
            return false;
        }
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([fact]);
        while let Some(current) = queue.pop_front() {
            if !seen.insert(current) {
                continue;
            }
            for parent in self.premises(current) {
                if *parent == premise {
                    return true;
                }
                queue.push_back(*parent);
            }
        }
        false
    }

    /// 已登记依赖的事实条数。
    pub fn len(&self) -> usize {
        self.deps.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.deps.is_empty()
    }
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "mgraph")
        .detail("operation", "proof_dependency")
        .detail("reason", reason)
}

#[cfg(test)]
mod tests {
    use super::{FactId, ProofDependencyIndex};

    #[test]
    fn records_and_queries_transitive_dependency() {
        let mut index = ProofDependencyIndex::new();
        index.record(FactId(1), &[]).unwrap();
        index.record(FactId(2), &[FactId(1)]).unwrap();
        index.record(FactId(3), &[FactId(2)]).unwrap();
        assert!(index.depends_on(FactId(3), FactId(1)));
        assert!(index.depends_on(FactId(3), FactId(2)));
        assert!(!index.depends_on(FactId(1), FactId(3)));
        assert_eq!(index.premises(FactId(3)), &[FactId(2)]);
    }

    #[test]
    fn rejects_self_and_future_premises() {
        let mut index = ProofDependencyIndex::new();
        let err = index.record(FactId(1), &[FactId(1)]).expect_err("self");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("self_dependency"));
        let err = index.record(FactId(1), &[FactId(2)]).expect_err("future");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("premise_not_prior"));
    }
}
