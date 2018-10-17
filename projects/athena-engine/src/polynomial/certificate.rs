//! Gröbner / 消元计算证书（可验证 metadata）。

use athena_types::RingId;

/// Gröbner 算法标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroebnerAlgorithm {
    /// Buchberger + 标准 S-pair 约化。
    Buchberger,
}

/// Gröbner / 消元结果证书。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroebnerCertificate {
    /// 算法。
    pub algorithm: GroebnerAlgorithm,
    /// 环 id。
    pub ring: RingId,
    /// 输入生成元数量。
    pub input_generators: usize,
    /// 输出基元素数量。
    pub basis_elements: usize,
    /// 执行的 S-pair 约化步数。
    pub s_pair_steps: u32,
    /// 是否在资源限制内完成（未截断）。
    pub complete: bool,
    /// 消元理想提取时保留的生成元数量（`None` = 非消元请求）。
    pub elimination_elements: Option<usize>,
}
