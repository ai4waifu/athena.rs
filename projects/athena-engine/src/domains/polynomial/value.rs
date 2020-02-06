//! 多项式域对外值句柄。

use super::{
    certificate::{GroebnerCertificate, GroebnerStatus},
    factor::PolynomialFactorization,
    groebner::{GroebnerComputation, GroebnerFrontier},
    modular_image::ModularImage,
    object::Polynomial,
};
use athena_types::RingId;

/// 擦除后的多项式句柄（不暴露泛型多项式）。
#[derive(Debug, Clone, PartialEq)]
pub struct PolynomialValue {
    /// 内部多项式对象。
    pub inner: Polynomial,
}

/// Gröbner / 消元基结果。
///
/// Partial / ResourceLimited 时保留 `pending_pairs` / `pending_insertion`，
/// 以便 Session 层诚实 resume（Living `30` G1）。
#[derive(Debug, Clone, PartialEq)]
pub struct GroebnerBasisValue {
    /// 所属环。
    pub ring: RingId,
    /// 基或候选多项式（canonical）。
    pub basis: Vec<Polynomial>,
    /// 计算证书。
    pub certificate: GroebnerCertificate,
    /// 显式状态分型（M-Graph admission 只接纳 [`GroebnerStatus::Verified`]）。
    pub status: GroebnerStatus,
    /// 尚未处理的 critical pairs（下标相对 `basis`）。
    pub pending_pairs: Vec<(usize, usize)>,
    /// 已算得但因基大小上限未能插入的多项式。
    pub pending_insertion: Option<Polynomial>,
}

impl GroebnerBasisValue {
    /// 从 [`GroebnerComputation`] 构造域值。
    pub fn from_computation(computation: GroebnerComputation) -> Self {
        match computation {
            GroebnerComputation::Complete(verified) => Self {
                ring: verified.ring,
                basis: verified.basis,
                certificate: verified.certificate,
                status: GroebnerStatus::Verified,
                pending_pairs: Vec::new(),
                pending_insertion: None,
            },
            GroebnerComputation::Partial(frontier) => Self::from_frontier(frontier, GroebnerStatus::Partial),
            GroebnerComputation::ResourceLimited(frontier) => Self::from_frontier(frontier, GroebnerStatus::ResourceLimited),
        }
    }

    fn from_frontier(frontier: GroebnerFrontier, status: GroebnerStatus) -> Self {
        Self {
            ring: frontier.ring,
            basis: frontier.candidates,
            certificate: frontier.certificate,
            status,
            pending_pairs: frontier.pending_pairs,
            pending_insertion: frontier.pending_insertion,
        }
    }

    /// 是否可作为 exact witness。
    pub fn is_exact_witness(&self) -> bool {
        self.status == GroebnerStatus::Verified && self.certificate.is_exact_witness()
    }

    /// 是否仍有可恢复 Buchberger 工作。
    pub fn has_resumable_work(&self) -> bool {
        self.status != GroebnerStatus::Verified && (self.pending_insertion.is_some() || !self.pending_pairs.is_empty())
    }

    /// 还原为 [`GroebnerFrontier`]（仅 Partial / ResourceLimited）。
    pub fn into_frontier(self) -> Option<GroebnerFrontier> {
        if self.status == GroebnerStatus::Verified {
            return None;
        }
        Some(GroebnerFrontier {
            ring: self.ring,
            candidates: self.basis,
            pending_pairs: self.pending_pairs,
            pending_insertion: self.pending_insertion,
            certificate: self.certificate,
        })
    }

    /// 经 DomainObject 仓物化为 [`super::request::PolynomialRequest::ResumeGroebner`]。
    pub fn to_resume_request(
        &self,
        store: &mut super::object_ref::PolynomialObjectStore,
        rings: &super::ring_table::RingTable,
        limits: super::groebner::GroebnerLimits,
    ) -> Option<super::request::PolynomialRequest> {
        if !self.has_resumable_work() {
            return None;
        }
        let candidates: Vec<_> = self.basis.iter().map(|p| store.intern(p.owning_copy(), rings)).collect();
        let pending_insertion = self.pending_insertion.as_ref().map(|p| store.intern(p.owning_copy(), rings));
        Some(super::request::PolynomialRequest::ResumeGroebner {
            candidates,
            pending_pairs: self.pending_pairs.clone(),
            pending_insertion,
            input_generators: self.certificate.input_generators,
            prior_s_pair_steps: self.certificate.s_pair_steps,
            limits,
        })
    }
}

/// 单变量除法结果值。
#[derive(Debug, Clone, PartialEq)]
pub struct UnivariateDivisionValue {
    /// 商。
    pub quotient: PolynomialValue,
    /// 余式。
    pub remainder: PolynomialValue,
}

/// 多项式域返回值。
#[derive(Debug, Clone, PartialEq)]
pub enum PolynomialDomainValue {
    /// 单个多项式。
    Polynomial(PolynomialValue),
    /// 单变量除法（商 + 余式）。
    UnivariateDivision(UnivariateDivisionValue),
    /// 因式分解（带完备性分型）。
    Factorization(PolynomialFactorization),
    /// Gröbner / 消元基。
    GroebnerBasis(GroebnerBasisValue),
    /// 模同态像（候选，不进 M-Graph admission）。
    ModularImage(ModularImage),
    /// 占位。
    Placeholder,
}
