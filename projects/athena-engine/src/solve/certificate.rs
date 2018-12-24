//! 证明与残差证书句柄。

use athena_types::TermId;

use crate::mgraph::WitnessRef;

/// 证明引用（详细载荷在 WitnessStore / claim 内联）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProofRef(pub WitnessRef);

impl ProofRef {
    /// 由 witness 构造。
    pub fn from_witness(witness: WitnessRef) -> Self {
        Self(witness)
    }
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
