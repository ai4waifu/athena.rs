//! 最优性种类与界证书。

use athena_types::ProofRef;

/// 最优性语义（`Optimal` 必须显式其一，禁止默认全局）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimalityKind {
    /// 全局最优（含凸问题全局）。
    Global,
    /// 局部最优。
    Local,
    /// 凸问题的全局最优。
    ConvexGlobal,
    /// 整数规划全局最优（gap=0 且有 integrality proof）。
    IntegerGlobal,
    /// 仅在给定容差内最优。
    ToleranceOptimal,
}

/// 证书种类（载荷待接入 verifier）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CertificateKind {
    /// LP 原/对偶可行 + strong duality。
    LinearStrongDuality,
    /// Farkas 不可行证书。
    FarkasInfeasibility,
    /// 凸二次/锥 KKT + complementarity + duality gap。
    ConvexKkt,
    /// MILP branch-and-bound 树摘要 + 上下界。
    BranchAndBound,
    /// NLP 局部 KKT（不得升格为全局）。
    LocalKkt,
    /// SOS / Positivstellensatz / 层次松弛。
    SosHierarchy,
    /// 尚未具体化的占位。
    Placeholder,
}

/// 界证书：上界/下界/不可行等可独立验证或明确降级的证据。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct BoundCertificate {
    /// 证书种类。
    pub kind: CertificateKind,
    /// 最优性语义（若声称最优）。
    pub optimality: Option<OptimalityKind>,
    /// 下界（若有）。
    pub lower_bound: Option<f64>,
    /// 上界 / incumbent 目标值（若有）。
    pub upper_bound: Option<f64>,
    /// 相对 gap（若有）。
    pub relative_gap: Option<f64>,
    /// 证明引用（可独立验证时填写）。
    pub proof: Option<ProofRef>,
    /// 人类可读摘要（非证明本体）。
    pub summary: String,
}

impl BoundCertificate {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            kind: self.kind,
            optimality: self.optimality,
            lower_bound: self.lower_bound,
            upper_bound: self.upper_bound,
            relative_gap: self.relative_gap,
            proof: self.proof,
            summary: self.summary.clone(),
        }
    }

    /// 占位证书（不可当作已验证最优）。
    pub fn placeholder(summary: impl Into<String>) -> Self {
        Self {
            kind: CertificateKind::Placeholder,
            optimality: None,
            lower_bound: None,
            upper_bound: None,
            relative_gap: None,
            proof: None,
            summary: summary.into(),
        }
    }
}
