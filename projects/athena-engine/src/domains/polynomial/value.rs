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
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct PolynomialValue {
    /// 内部多项式对象。
    pub inner: Polynomial,
}

impl PolynomialValue {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { inner: self.inner.owning_copy() }
    }
}

/// Gröbner / 消元基结果。
///
/// Partial / ResourceLimited 时保留 `pending_pairs` / `pending_insertion`，
/// 以便 Session 层诚实 resume（G1）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
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
    /// F4 候选 sugar（与 `basis` 等长）。Buchberger 为 `None`。
    pub candidate_sugars: Option<Vec<u32>>,
    /// 待插入多项式的 sugar（仅 F4 ResourceLimited）。
    pub pending_insertion_sugar: Option<u32>,
}

impl GroebnerBasisValue {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            ring: self.ring,
            basis: self.basis.iter().map(Polynomial::owning_copy).collect(),
            certificate: self.certificate.owning_copy(),
            status: self.status,
            pending_pairs: self.pending_pairs.clone(),
            pending_insertion: self.pending_insertion.as_ref().map(Polynomial::owning_copy),
            candidate_sugars: self.candidate_sugars.clone(),
            pending_insertion_sugar: self.pending_insertion_sugar,
        }
    }

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
                candidate_sugars: None,
                pending_insertion_sugar: None,
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
            candidate_sugars: frontier.candidate_sugars,
            pending_insertion_sugar: frontier.pending_insertion_sugar,
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
            candidate_sugars: self.candidate_sugars,
            pending_insertion_sugar: self.pending_insertion_sugar,
            certificate: self.certificate,
        })
    }

    /// 若仍有可恢复工作，登记统一 [`crate::runtime::FrontierStore`] 条目并返回身份。
    pub fn register_frontier_on_session(
        &self,
        session: &mut crate::runtime::Session,
        goal_fingerprint: u64,
    ) -> Option<athena_types::FrontierId> {
        if !self.has_resumable_work() {
            return None;
        }
        use crate::{
            domains::solve::{ResumeKind, ResumeToken},
            runtime::{ComputationFrontier, results::ResultProviderId},
        };
        let stamp = ResultProviderId::POLYNOMIAL.stamped();
        let resume = ResumeToken::empty_with_provider(ResumeKind::Groebner, stamp);
        let mut record = ComputationFrontier::new(goal_fingerprint, resume);
        record.algorithm = Some(match self.certificate.algorithm {
            super::certificate::GroebnerAlgorithm::F4 => "groebner_f4",
            super::certificate::GroebnerAlgorithm::Buchberger => "groebner_buchberger",
        });
        record.budget_consumed = u64::from(self.certificate.s_pair_steps);
        Some(session.insert_frontier(record))
    }

    /// 经 DomainObject 仓物化为 ResumeBuchberger / ResumeF4 请求。
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
        match self.certificate.algorithm {
            super::certificate::GroebnerAlgorithm::F4 => Some(super::request::PolynomialRequest::ResumeGroebnerF4 {
                candidates,
                pending_pairs: self.pending_pairs.clone(),
                pending_insertion,
                input_generators: self.certificate.input_generators,
                prior_s_pair_steps: self.certificate.s_pair_steps,
                candidate_sugars: self.candidate_sugars.clone(),
                pending_insertion_sugar: self.pending_insertion_sugar,
                limits,
            }),
            super::certificate::GroebnerAlgorithm::Buchberger => Some(super::request::PolynomialRequest::ResumeGroebner {
                candidates,
                pending_pairs: self.pending_pairs.clone(),
                pending_insertion,
                input_generators: self.certificate.input_generators,
                prior_s_pair_steps: self.certificate.s_pair_steps,
                limits,
            }),
        }
    }
}

/// 单变量除法结果值。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct UnivariateDivisionValue {
    /// 商。
    pub quotient: PolynomialValue,
    /// 余式。
    pub remainder: PolynomialValue,
}

impl UnivariateDivisionValue {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { quotient: self.quotient.owning_copy(), remainder: self.remainder.owning_copy() }
    }
}

/// 多项式域返回值。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
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

impl PolynomialDomainValue {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Polynomial(v) => Self::Polynomial(v.owning_copy()),
            Self::UnivariateDivision(v) => Self::UnivariateDivision(v.owning_copy()),
            Self::Factorization(v) => Self::Factorization(v.owning_copy()),
            Self::GroebnerBasis(v) => Self::GroebnerBasis(v.owning_copy()),
            Self::ModularImage(v) => Self::ModularImage(v.owning_copy()),
            Self::Placeholder => Self::Placeholder,
        }
    }
}
