//! Reflector 合同（调度侧骨架）。
//!
//! Living [`29`]：语义缺口驱动请用 [`crate::reasoning::mgraph::SemanticReflector`] /
//! [`crate::reasoning::mgraph::Reflection`]。本文件的 [`Reflector`] / [`ReflectionResult`]
//! 是旧 solver 调度合同，不得直接写 admitted relation；后续切片迁入 SemanticReflector。

use athena_types::{Diagnostic, TermId};

use crate::reasoning::mgraph::{DeterminacyGuarantee, EqualityWitness, HyperEdge, MGraphState};

use super::{
    request::SolverRequest,
    types::{CapabilityProviderId, SolverMetadata},
};

/// 求解上下文。
#[derive(Debug, Clone, Copy)]
pub struct SolverContext {
    /// 能力 provider。
    pub provider: CapabilityProviderId,
}

/// Reflection 结果 — solver 不直改 `TermStore`。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct ReflectionResult {
    /// 等式。
    pub equalities: Vec<EqualityWitness>,
    /// 超边。
    pub hyper_edges: Vec<HyperEdge>,
    /// 确定性保证。
    pub guarantees: Vec<DeterminacyGuarantee>,
    /// 残差项。
    pub residual: Option<TermId>,
    /// 元数据。
    pub metadata: SolverMetadata,
}

impl ReflectionResult {
    /// 空结果。
    pub fn empty() -> Self {
        Self { equalities: Vec::new(), hyper_edges: Vec::new(), guarantees: Vec::new(), residual: None, metadata: SolverMetadata::default() }
    }

    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            equalities: self.equalities.iter().map(EqualityWitness::owning_copy).collect(),
            hyper_edges: self.hyper_edges.iter().map(HyperEdge::owning_copy).collect(),
            guarantees: self.guarantees.clone(),
            residual: self.residual,
            metadata: self.metadata.owning_copy(),
        }
    }
}

/// 单向 reflector。
pub trait Reflector: Send + Sync {
    /// 对当前状态反射。
    fn reflect(&self, state: &MGraphState, request: &SolverRequest, context: &SolverContext) -> Result<ReflectionResult, Diagnostic>;
}
