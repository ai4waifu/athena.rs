//! 可恢复搜索 frontier（incumbent / best bound / 节点状态）。
//!
//! branch-and-bound 必须同时保留 incumbent、best bound 与 gap。
//! 资源耗尽不得把 incumbent 包装成 `Optimal`。

use super::{certificate::BoundCertificate, fingerprint::OptimizationFingerprint};

/// 优化搜索前沿（骨架）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct OptimizationFrontier {
    /// 绑定的问题指纹。
    pub problem_fingerprint: OptimizationFingerprint,
    /// 算法策略名。
    pub algorithm: String,
    /// 算法版本。
    pub algorithm_version: u32,
    /// 随机种子（若有）。
    pub random_seed: Option<u64>,
    /// 当前 incumbent 目标值（若有）。
    pub incumbent_value: Option<f64>,
    /// 当前 best bound（若有）。
    pub best_bound: Option<f64>,
    /// 相对 gap（若有）。
    pub relative_gap: Option<f64>,
    /// 已验证证书摘要。
    pub certificates: Vec<BoundCertificate>,
    /// 不透明恢复令牌（后续接序列化）。
    pub resume_token: Option<Vec<u8>>,
}

impl OptimizationFrontier {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            problem_fingerprint: self.problem_fingerprint,
            algorithm: self.algorithm.clone(),
            algorithm_version: self.algorithm_version,
            random_seed: self.random_seed,
            incumbent_value: self.incumbent_value,
            best_bound: self.best_bound,
            relative_gap: self.relative_gap,
            certificates: self.certificates.iter().map(BoundCertificate::owning_copy).collect(),
            resume_token: self.resume_token.clone(),
        }
    }

    /// 空前沿。
    pub fn empty(problem_fingerprint: OptimizationFingerprint, algorithm: impl Into<String>, algorithm_version: u32) -> Self {
        Self {
            problem_fingerprint,
            algorithm: algorithm.into(),
            algorithm_version,
            random_seed: None,
            incumbent_value: None,
            best_bound: None,
            relative_gap: None,
            certificates: Vec::new(),
            resume_token: None,
        }
    }
}
