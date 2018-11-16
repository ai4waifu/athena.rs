//! 数论结果对象（非裸整数列表）。

use athena_numeric::{Integer, ModularValue};

/// Miller–Rabin 基选择策略。固定基可复现，但**不是**独立随机样本，
/// 不得按通常随机见证假设计算误判概率上界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MillerRabinBaseSelection {
    /// 固定确定性序列（截断自引擎内建小素数表）。
    Fixed,
}

/// 素性判定结果 — 禁止把 Miller-Rabin probable 写成确定 `true`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Primality {
    /// 确定素数（试除，或已证明覆盖输入上界的确定性见证集）。
    Prime,
    /// 确定合数。
    Composite,
    /// 概率素数；证据只记录**实际执行**的基与选择策略。
    ProbablePrime {
        /// 实际测试的基（按执行顺序）。
        bases: Vec<u32>,
        /// 基如何选取。
        base_selection: MillerRabinBaseSelection,
        /// 实际执行的基数量（等于 `bases.len()`）。
        rounds_executed: u32,
    },
    /// 未判定（例如请求 0 轮且无确定性路径）。
    Unknown,
}

/// 素幂因子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimePower {
    /// 底数（素数或未证素数基；完整性见外层 `FactorizationCompleteness`）。
    pub base: Integer,
    /// 指数。
    pub exponent: u32,
}

/// 整数分解完整性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactorizationCompleteness {
    /// 完全分解为确定素因子。
    Complete,
    /// 因子仅为概率素数（余因子为 1）。
    Probable,
    /// 仍有合数余因子。
    Partial,
    /// 触及试除 / 比特资源上限。
    ResourceLimited,
}

/// 带完整性的整数分解对象。
///
/// 不变量（非零输入）：`input = unit * Π base_i^e_i * remainder`，`unit ∈ {-1,1}`，
/// `base_i > 1`，`e_i > 0`。`0` 不进入本结构（见 `factor_integer` 域错误）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Factorization {
    /// 单位（符号：`±1`）。
    pub unit: Integer,
    /// 素幂因子（升序底）。
    pub factors: Vec<PrimePower>,
    /// 未分解余因子（`Partial` / `ResourceLimited` 时可能非 1）。
    pub remainder: Integer,
    /// 完整性。
    pub completeness: FactorizationCompleteness,
}

/// 扩展欧几里得：`s·a + t·b = g`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedGcd {
    /// `gcd(|a|,|b|)`（非负）。
    pub g: Integer,
    /// Bézout `s`。
    pub s: Integer,
    /// Bézout `t`。
    pub t: Integer,
}

/// 数论域值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberTheoryValue {
    /// 整数标量（gcd / lcm 等）。
    Integer(Integer),
    /// 扩展 gcd。
    ExtendedGcd(ExtendedGcd),
    /// 素性。
    Primality(Primality),
    /// 分解。
    Factorization(Factorization),
    /// 模运算结果。
    Modular(ModularValue),
}
