//! 数论结果对象（非裸整数列表）。

use athena_numeric::{Integer, ModularValue};

/// 素性判定结果 — 禁止把 Miller-Rabin probable 写成确定 `true`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Primality {
    /// 确定素数（试除或确定性见证集）。
    Prime,
    /// 确定合数。
    Composite,
    /// 概率素数；`rounds` 为独立 Miller-Rabin 轮数。
    ProbablePrime {
        /// 见证轮数。
        rounds: u32,
    },
    /// 未判定（资源不足等）。
    Unknown,
}

/// 素幂因子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimePower {
    /// 底数（素数或未证素数基）。
    pub base: Integer,
    /// 指数。
    pub exponent: u32,
}

/// 整数分解完整性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactorizationCompleteness {
    /// 完全分解为确定素因子。
    Complete,
    /// 因子仅为概率素数。
    Probable,
    /// 仍有合数余因子。
    Partial,
    /// 触及试除 / 比特资源上限。
    ResourceLimited,
}

/// 带完整性的整数分解对象。
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
