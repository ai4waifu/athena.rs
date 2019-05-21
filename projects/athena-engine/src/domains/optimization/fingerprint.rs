//! 问题 fingerprint（稳定身份；`ProblemId` 只是 Session 句柄）。

/// 优化问题稳定指纹（占位：完整 canonical 编码待接入）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OptimizationFingerprint {
    /// 64-bit digest（算法版本变更必须换键）。
    pub digest: u64,
    /// 指纹算法标识。
    pub algorithm: &'static str,
}

/// 当前指纹算法名（缓存键组成部分）。
pub const FINGERPRINT_ALGORITHM: &str = "athena-opt-fp-v0-placeholder";

/// 构造占位指纹（**不是**可缓存的稳定合同；仅骨架连通）。
pub fn fingerprint_placeholder(seed: u64) -> OptimizationFingerprint {
    OptimizationFingerprint { digest: seed, algorithm: FINGERPRINT_ALGORITHM }
}
