//! 舍入策略。

/// 舍入策略（骨架）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingPolicy {
    /// 最近偶数。
    #[default]
    NearestEven,
    /// 向零。
    TowardZero,
    /// 向 +∞。
    TowardPosInf,
    /// 向 −∞。
    TowardNegInf,
}
