//! Gröbner / 消元计算证书与验证状态。

use athena_types::RingId;

use crate::domains::algebra::{PropertyState, PropertyWitness};

/// Gröbner 算法标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroebnerAlgorithm {
    /// Buchberger + 标准 S-pair 约化。
    Buchberger,
}

/// 计算终态（与 [`super::groebner::GroebnerComputation`] 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroebnerStatus {
    /// 资源内完成且通过独立 verifier。
    Verified,
    /// S-pair 预算耗尽；候选不可作数学证书。
    Partial,
    /// 基大小等硬资源上限；候选不可作数学证书。
    ResourceLimited,
}

/// Gröbner / 消元结果证书（运行统计 + 验证状态）。
///
/// 禁止薄 `verified: bool`：独立 verifier 结果用 [`PropertyState`]。
#[derive(Debug, Clone, PartialEq)]
pub struct GroebnerCertificate {
    /// 算法。
    pub algorithm: GroebnerAlgorithm,
    /// 环 id。
    pub ring: RingId,
    /// 输入生成元数量。
    pub input_generators: usize,
    /// 输出基 / 候选元素数量。
    pub basis_elements: usize,
    /// 执行的 S-pair 约化步数。
    pub s_pair_steps: u32,
    /// 是否在 S-pair 资源限制内跑完 Buchberger 主循环。
    pub complete: bool,
    /// 独立 verifier 结果（`Proven` 才可作 exact witness）。
    pub verification: PropertyState<()>,
    /// 消元理想提取时保留的生成元数量（`None` = 非消元请求）。
    pub elimination_elements: Option<usize>,
}

impl GroebnerCertificate {
    /// 独立 verifier 已通过。
    pub fn mark_verified(&mut self) {
        self.verification = PropertyState::Proven { value: (), witness: PropertyWitness::placeholder("groebner_independent_verifier") };
    }

    /// 清除验证态（partial / resource-limited 候选）。
    pub fn mark_unverified(&mut self) {
        self.verification = PropertyState::Unknown;
    }

    /// 是否可作为 M-Graph exact witness。
    pub fn is_exact_witness(&self) -> bool {
        self.complete && self.verification.is_proven()
    }
}
