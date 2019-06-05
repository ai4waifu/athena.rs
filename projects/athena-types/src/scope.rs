//! 假设作用域。
//!
//! 一等对象：合并、冲突检测、继承、投影。禁止各模块平行维护 `complete: bool`。

use crate::{
    Precision,
    assumption::{AssumptionSet, Predicate},
    ids::{AssumptionScopeId, AssumptionSetId, DomainId, SymbolId, TheoryContextId},
};

/// 理论 / 猜想上下文（与 Living `23` ANT0 对齐，禁止平行字符串假设）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum TheoryContext {
    /// 经典无额外猜想。
    #[default]
    ClassicalUnconditional,
    /// 广义黎曼假设。
    UnderGRH,
    /// 黎曼假设。
    UnderRH,
    /// Schanuel 猜想。
    UnderSchanuel,
    /// 广义 ABC。
    UnderGeneralizedABC,
    /// 选择公理依赖。
    UnderChoiceAxiom,
    /// 其它已登记猜想 / 理论上下文。
    ConditionalOnConjecture(TheoryContextId),
    /// 自定义上下文句柄。
    Custom(TheoryContextId),
}

/// 假设作用域上的分支策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AssumptionBranchPolicy {
    /// 主值 / 主分支。
    #[default]
    Principal,
    /// 保留全部分支。
    AllBranches,
    /// 显式受限（细节由谓词表达）。
    Restricted,
}

/// 结果相对假设的适用性（不得压成 `bool`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScopeApplicability {
    /// 无条件成立。
    Unconditional,
    /// 在给定作用域下成立。
    Conditional {
        /// 作用域。
        scope: AssumptionScopeId,
    },
    /// 概率性。
    Probable,
    /// 数值证书。
    NumericallyCertified,
    /// 未知。
    Unknown,
    /// 假设自相矛盾。
    ContradictoryAssumptions,
}

/// 冲突种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeConflictKind {
    /// 直接相反谓词。
    PredicateContradiction,
    /// 理论上下文互斥。
    TheoryContextMismatch,
    /// 系数域不一致。
    DomainMismatch,
    /// 精度策略不一致。
    PrecisionMismatch,
}

/// 作用域冲突描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeConflict {
    /// 种类。
    pub kind: ScopeConflictKind,
    /// 左侧谓词（若有）。
    pub left: Option<Predicate>,
    /// 右侧谓词（若有）。
    pub right: Option<Predicate>,
}

/// 合并结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeMergeOutcome {
    /// 合并成功。
    Ok(AssumptionScope),
    /// 冲突。
    Conflict(ScopeConflict),
}

/// 假设作用域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumptionScope {
    /// Session 内稳定 id（未 intern 时为 `None`）。
    pub id: Option<AssumptionScopeId>,
    /// 父作用域（继承链）。
    pub parent: Option<AssumptionScopeId>,
    /// 本层谓词。
    pub predicates: Vec<Predicate>,
    /// 理论上下文。
    pub theory_context: TheoryContext,
    /// 分支策略。
    pub branch_policy: AssumptionBranchPolicy,
    /// 系数域（可选显式）。
    pub coefficient_domain: Option<DomainId>,
    /// 精度策略（可选显式）。
    pub precision_policy: Option<Precision>,
}

impl Default for AssumptionScope {
    fn default() -> Self {
        Self::unconditional()
    }
}

impl AssumptionScope {
    /// 无条件空作用域。
    pub fn unconditional() -> Self {
        Self {
            id: None,
            parent: None,
            predicates: Vec::new(),
            theory_context: TheoryContext::ClassicalUnconditional,
            branch_policy: AssumptionBranchPolicy::Principal,
            coefficient_domain: None,
            precision_policy: None,
        }
    }

    /// 由谓词构造（无父作用域）。
    pub fn from_predicates(predicates: Vec<Predicate>) -> Self {
        Self { predicates, ..Self::unconditional() }
    }

    /// 从过渡期 [`AssumptionSet`] 提升（保留谓词；id 数值载荷可对齐）。
    pub fn from_assumption_set(set: &AssumptionSet) -> Self {
        Self {
            id: set.id.map(|AssumptionSetId(v)| AssumptionScopeId(v)),
            parent: None,
            predicates: set.predicates.clone(),
            theory_context: TheoryContext::ClassicalUnconditional,
            branch_policy: AssumptionBranchPolicy::Principal,
            coefficient_domain: None,
            precision_policy: None,
        }
    }

    /// 回落为过渡期 [`AssumptionSet`]（丢失父链 / 理论 / 分支 / 域字段）。
    pub fn to_assumption_set(&self) -> AssumptionSet {
        AssumptionSet { id: self.id.map(|AssumptionScopeId(v)| AssumptionSetId(v)), predicates: self.predicates.clone() }
    }

    /// 是否无本层谓词、无父、经典无猜想。
    pub fn is_unconditional_empty(&self) -> bool {
        self.parent.is_none() && self.predicates.is_empty() && matches!(self.theory_context, TheoryContext::ClassicalUnconditional)
    }

    /// 继承：以 `parent` 为父，附加本层谓词。
    pub fn inherit(parent: AssumptionScopeId, local: Vec<Predicate>) -> Self {
        Self { id: None, parent: Some(parent), predicates: local, ..Self::unconditional() }
    }

    /// 在已知祖先表上展开全部谓词（根 → 叶）。
    pub fn inherited_predicates<F>(&self, mut resolve: F) -> Vec<Predicate>
    where
        F: FnMut(AssumptionScopeId) -> Option<AssumptionScope>,
    {
        let mut chain = Vec::new();
        let mut current_parent = self.parent;
        let mut guard = 0u32;
        while let Some(pid) = current_parent {
            if guard > 10_000 {
                break;
            }
            guard += 1;
            let Some(parent) = resolve(pid)
            else {
                break;
            };
            chain.push(parent.predicates.clone());
            current_parent = parent.parent;
        }
        chain.reverse();
        let mut out = Vec::new();
        for preds in chain {
            out.extend(preds);
        }
        out.extend(self.predicates.iter().cloned());
        out
    }

    /// 投影：仅保留与给定符号相关的符号级谓词。
    ///
    /// 项级谓词在符号未知时保守丢弃，避免误保留。
    pub fn project_to_symbols(&self, symbols: &[SymbolId]) -> Self {
        let keep = |p: &Predicate| match p {
            Predicate::SymbolNonZero(s) | Predicate::SymbolReal(s) => symbols.contains(s),
            _ => false,
        };
        Self {
            id: None,
            parent: None,
            predicates: self.predicates.iter().filter(|p| keep(p)).cloned().collect(),
            theory_context: self.theory_context.clone(),
            branch_policy: self.branch_policy,
            coefficient_domain: self.coefficient_domain,
            precision_policy: self.precision_policy,
        }
    }

    /// 合并两个作用域（谓词并集 + 冲突检测）。
    pub fn merge(&self, other: &Self) -> ScopeMergeOutcome {
        if let Some(conflict) = theory_context_conflict(&self.theory_context, &other.theory_context) {
            return ScopeMergeOutcome::Conflict(conflict);
        }
        let mut predicates = self.predicates.clone();
        for p in &other.predicates {
            if !predicates.contains(p) {
                predicates.push(p.clone());
            }
        }
        if let Some(conflict) = detect_predicate_conflict(&predicates) {
            return ScopeMergeOutcome::Conflict(conflict);
        }
        let parent = match (self.parent, other.parent) {
            (None, None) => None,
            (Some(a), None) | (None, Some(a)) => Some(a),
            (Some(a), Some(b)) if a == b => Some(a),
            (Some(_), Some(_)) => None,
        };
        let coefficient_domain = match (self.coefficient_domain, other.coefficient_domain) {
            (None, x) | (x, None) => x,
            (Some(a), Some(b)) if a == b => Some(a),
            (Some(_), Some(_)) => {
                return ScopeMergeOutcome::Conflict(ScopeConflict { kind: ScopeConflictKind::DomainMismatch, left: None, right: None });
            }
        };
        let precision_policy = match (self.precision_policy, other.precision_policy) {
            (None, x) | (x, None) => x,
            (Some(a), Some(b)) if a == b => Some(a),
            (Some(_), Some(_)) => {
                return ScopeMergeOutcome::Conflict(ScopeConflict { kind: ScopeConflictKind::PrecisionMismatch, left: None, right: None });
            }
        };
        ScopeMergeOutcome::Ok(Self {
            id: None,
            parent,
            predicates,
            theory_context: merge_theory_context(&self.theory_context, &other.theory_context),
            branch_policy: merge_branch_policy(self.branch_policy, other.branch_policy),
            coefficient_domain,
            precision_policy,
        })
    }

    /// 仅检测本层谓词冲突。
    pub fn local_conflict(&self) -> Option<ScopeConflict> {
        detect_predicate_conflict(&self.predicates)
    }
}

fn merge_branch_policy(a: AssumptionBranchPolicy, b: AssumptionBranchPolicy) -> AssumptionBranchPolicy {
    use AssumptionBranchPolicy::*;
    match (a, b) {
        (AllBranches, _) | (_, AllBranches) => AllBranches,
        (Restricted, _) | (_, Restricted) => Restricted,
        (Principal, Principal) => Principal,
    }
}

fn merge_theory_context(a: &TheoryContext, b: &TheoryContext) -> TheoryContext {
    use TheoryContext::*;
    match (a, b) {
        (ClassicalUnconditional, x) | (x, ClassicalUnconditional) => x.clone(),
        // GRH 蕴涵 RH：合并保留更强假设。
        (UnderGRH, UnderRH) | (UnderRH, UnderGRH) => UnderGRH,
        (x, y) if x == y => x.clone(),
        (x, _) => x.clone(),
    }
}

fn theory_context_conflict(a: &TheoryContext, b: &TheoryContext) -> Option<ScopeConflict> {
    use TheoryContext::*;
    let ok = match (a, b) {
        (ClassicalUnconditional, _) | (_, ClassicalUnconditional) => true,
        (x, y) if x == y => true,
        (UnderGRH, UnderRH) | (UnderRH, UnderGRH) => true,
        _ => false,
    };
    if ok { None } else { Some(ScopeConflict { kind: ScopeConflictKind::TheoryContextMismatch, left: None, right: None }) }
}

fn detect_predicate_conflict(predicates: &[Predicate]) -> Option<ScopeConflict> {
    for (i, left) in predicates.iter().enumerate() {
        for right in predicates.iter().skip(i + 1) {
            if predicates_contradict(left, right) {
                return Some(ScopeConflict {
                    kind: ScopeConflictKind::PredicateContradiction,
                    left: Some(left.clone()),
                    right: Some(right.clone()),
                });
            }
        }
    }
    None
}

fn predicates_contradict(a: &Predicate, b: &Predicate) -> bool {
    use Predicate::*;
    match (a, b) {
        (Equal(x1, y1), NotEqual(x2, y2)) | (NotEqual(x1, y1), Equal(x2, y2)) => (*x1 == *x2 && *y1 == *y2) || (*x1 == *y2 && *y1 == *x2),
        _ => false,
    }
}
