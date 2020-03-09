//! 素性 / 分解可验证证据（verifier-friendly；生成器可后续扩展）。

use crate::runtime::values::numeric_clone::clone_integer;
use athena_numeric::Integer;

use super::value::MillerRabinBaseSelection;

/// 有明确数值上界的确定性素性测试证书。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimeCertificate {
    /// 试除确定性路径（含上界）。
    TrialDivision {
        /// 试除上界（含）。
        bound: u64,
    },
    /// 强 Miller–Rabin 固定见证集覆盖的上界（如全 `u64`）。
    DeterministicMillerRabin {
        /// 覆盖的最大比特数（例如 64）。
        max_value_bits: u32,
        /// 实际见证基。
        witnesses: Vec<u32>,
    },
    /// 小整数特殊情形（如 2、3）。
    SmallPrime,
}

/// 强 Miller–Rabin 概率素数证据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbablePrimeEvidence {
    /// 实际测试的基（按执行顺序）。
    pub bases: Vec<u32>,
    /// 基如何选取。
    pub base_selection: MillerRabinBaseSelection,
    /// 实际执行的基数量。
    pub rounds_executed: u32,
}

impl ProbablePrimeEvidence {
    /// 由固定基 MR 路径构造。
    pub fn fixed(bases: Vec<u32>) -> Self {
        let rounds_executed = bases.len() as u32;
        Self { bases, base_selection: MillerRabinBaseSelection::Fixed, rounds_executed }
    }
}

/// 合数可验证见证。
#[derive(Debug, PartialEq, Eq)]
pub enum CompositeWitness {
    /// 非正或 `1`（非素数定义）。
    NonPositiveOrOne,
    /// 偶数 / 被 2 整除。
    Even,
    /// 发现非平凡因子（`1 < d < n`）。
    SmallFactor {
        /// 非平凡因子。
        divisor: Integer,
    },
    /// 强 Miller–Rabin 见证（`1 < a < n`）。
    MillerRabin {
        /// 见证基。
        base: u32,
    },
}

impl CompositeWitness {
    /// Owning 复制（Living `31`：禁止默认 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::NonPositiveOrOne => Self::NonPositiveOrOne,
            Self::Even => Self::Even,
            Self::SmallFactor { divisor } => Self::SmallFactor { divisor: clone_integer(divisor) },
            Self::MillerRabin { base } => Self::MillerRabin { base: *base },
        }
    }
}
