//! 覆盖状态映射（disposition / factorization → [`CoverageStatus`]）。

use crate::domains::{
    linear_algebra::{AlgorithmGuarantee, SolveDisposition},
    polynomial::PolynomialFactorizationCompleteness,
};

use super::{
    coverage::CoverageStatus,
    frontier::{ResumeKind, ResumeToken},
};

/// 精确线性求解 disposition → 覆盖。
pub fn coverage_from_exact_disposition(disposition: &SolveDisposition) -> CoverageStatus {
    match disposition {
        SolveDisposition::Unique | SolveDisposition::Inconsistent => CoverageStatus::Complete,
        // 仅有特解与自由列下标，尚无零空间基，不得声称完整参数族。
        SolveDisposition::Infinite { .. } => CoverageStatus::CertifiedSubset,
        SolveDisposition::Singular => CoverageStatus::Unsupported,
        SolveDisposition::ResourceLimited => CoverageStatus::ResourceLimited { frontier: ResumeToken::empty(ResumeKind::LinearExact) },
    }
}

/// 机器线性求解 disposition → 覆盖（永不进入 exact union-find）。
pub fn coverage_from_machine_disposition(disposition: &SolveDisposition, guarantee: AlgorithmGuarantee) -> CoverageStatus {
    let _ = guarantee;
    match disposition {
        SolveDisposition::Unique => CoverageStatus::LocalOnly,
        SolveDisposition::Singular | SolveDisposition::Inconsistent => CoverageStatus::LocalOnly,
        SolveDisposition::Infinite { .. } => CoverageStatus::CertifiedSubset,
        SolveDisposition::ResourceLimited => CoverageStatus::ResourceLimited { frontier: ResumeToken::empty(ResumeKind::LinearMachine) },
    }
}

/// 一元因式分解完整性 → 根集覆盖。
pub fn coverage_from_factorization(completeness: PolynomialFactorizationCompleteness) -> CoverageStatus {
    match completeness {
        // 仅当调用方确认全部因子均为一次时，才可升级为 Complete（见 univariate adapter）。
        PolynomialFactorizationCompleteness::Complete => CoverageStatus::Complete,
        PolynomialFactorizationCompleteness::Probable => CoverageStatus::Probable,
        PolynomialFactorizationCompleteness::Partial => CoverageStatus::CertifiedSubset,
        PolynomialFactorizationCompleteness::ResourceLimited => {
            CoverageStatus::ResourceLimited { frontier: ResumeToken::empty(ResumeKind::UnivariateFactor) }
        }
    }
}
