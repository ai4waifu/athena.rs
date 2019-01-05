//! 解集与分支。

use athena_types::ProofRef;

use super::{
    binding::{BindingMap, BoundSymbol},
    certificate::ResidualCertificate,
    constraint::ConstraintSet,
    coverage::CoverageStatus,
    domain::SolveDomain,
    frontier::ResumeToken,
};

/// 分支状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BranchStatus {
    /// 候选，尚未 verifier 接纳。
    Candidate,
    /// 已验证满足问题。
    Verified,
    /// 带条件成立。
    Conditional,
    /// 已拒绝。
    Rejected,
}

/// 重数信息（不可去重后冒充完整）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiplicityInfo {
    /// 代数重数（若已知）。
    pub algebraic: Option<u32>,
    /// 几何重数（若已知）。
    pub geometric: Option<u32>,
}

/// 单个解分支。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolutionBranch {
    /// 变量绑定。
    pub bindings: BindingMap,
    /// 分支成立条件。
    pub conditions: ConstraintSet,
    /// 重数。
    pub multiplicity: Option<MultiplicityInfo>,
    /// 分支状态。
    pub status: BranchStatus,
}

impl SolutionBranch {
    /// 空条件、无重数的候选分支。
    pub fn candidate(bindings: BindingMap) -> Self {
        Self { bindings, conditions: ConstraintSet::empty_and(), multiplicity: None, status: BranchStatus::Candidate }
    }
}

/// 统一解集模型（不是 `Vec<Binding>`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolutionSet {
    /// 解集涉及的变量（通常对齐 problem.unknowns）。
    pub variables: Vec<BoundSymbol>,
    /// 分支。
    pub branches: Vec<SolutionBranch>,
    /// 覆盖承诺。
    pub coverage: CoverageStatus,
    /// 结果域。
    pub domain: SolveDomain,
    /// 完整性 / 等价性证明引用。
    pub proof: Option<ProofRef>,
    /// 残差证书。
    pub residual: Option<ResidualCertificate>,
    /// 可恢复前沿（亦可嵌在 `CoverageStatus::ResourceLimited`）。
    pub frontier: Option<ResumeToken>,
}

impl SolutionSet {
    /// 空解集且声明完整（无解的已证情形仍需单独 proof）。
    pub fn empty_complete(variables: Vec<BoundSymbol>, domain: SolveDomain) -> Self {
        Self {
            variables,
            branches: Vec::new(),
            coverage: CoverageStatus::Complete,
            domain,
            proof: None,
            residual: None,
            frontier: None,
        }
    }

    /// 局部-only 单分支包装（`FindRoot` / `fsolve` 路径）。
    pub fn local_only(variables: Vec<BoundSymbol>, domain: SolveDomain, branch: SolutionBranch) -> Self {
        Self {
            variables,
            branches: vec![branch],
            coverage: CoverageStatus::LocalOnly,
            domain,
            proof: None,
            residual: None,
            frontier: None,
        }
    }

    /// 模型查找子集（`FindInstance`，不得冒充完整）。
    pub fn certified_subset(variables: Vec<BoundSymbol>, domain: SolveDomain, branches: Vec<SolutionBranch>) -> Self {
        Self {
            variables,
            branches,
            coverage: CoverageStatus::CertifiedSubset,
            domain,
            proof: None,
            residual: None,
            frontier: None,
        }
    }

    /// 是否允许进入 exact union-find。
    pub fn admits_exact_union_find(&self) -> bool {
        self.coverage.admits_exact_union_find()
    }
}
