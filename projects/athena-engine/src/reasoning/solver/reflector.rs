//! Reflector 合同。

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
#[derive(Debug, Clone, PartialEq, Eq)]
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
        Self {
            equalities: Vec::new(),
            hyper_edges: Vec::new(),
            guarantees: Vec::new(),
            residual: None,
            metadata: SolverMetadata::default(),
        }
    }
}

/// 单向 reflector。
pub trait Reflector: Send + Sync {
    /// 对当前状态反射。
    fn reflect(
        &self,
        state: &MGraphState,
        request: &SolverRequest,
        context: &SolverContext,
    ) -> Result<ReflectionResult, Diagnostic>;
}
