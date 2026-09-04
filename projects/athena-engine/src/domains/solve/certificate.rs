//! 残差证书句柄。
//!
//! 证明身份统一为 [`athena_types::ProofRef`]。

use athena_types::{ProofRef, TermId};

use crate::reasoning::mgraph::WitnessRef;

/// 由 M-Graph [`WitnessRef`] 构造证明引用。
pub fn proof_ref_from_witness(witness: WitnessRef) -> ProofRef {
    ProofRef(witness.0)
}

/// 残差 / 回代证书（结构化，非「残差通过 = 唯一」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualCertificate {
    /// 残差项（可为向量残差的编码根）。
    pub residual: TermId,
    /// 是否数值/符号上判定为可接受零。
    pub residual_is_zero: bool,
    /// 可选条件估计标签。
    pub condition_note: Option<String>,
}
