//! 数论域请求 — 宿主传入已解码整数 / 模数。

use athena_numeric::{Integer, Modulus};

use super::factor::FactorLimits;
use crate::runtime::values::numeric_clone::{clone_integer, clone_integers, clone_modulus};

/// 数论域请求 — 宿主传入已解码整数 / 模数。
#[derive(Debug, PartialEq, Eq)]
pub enum NumberTheoryRequest {
    /// `gcd(a, b)`（非负）。
    Gcd {
        /// 左操作数。
        a: Integer,
        /// 右操作数。
        b: Integer,
    },
    /// `lcm(a, b)`（非负；`0` 约定 `lcm(0,0)=0`）。
    Lcm {
        /// 左操作数。
        a: Integer,
        /// 右操作数。
        b: Integer,
    },
    /// extended Euclidean。
    ExtendedGcd {
        /// `a`。
        a: Integer,
        /// `b`。
        b: Integer,
    },
    /// 素性测试。
    PrimalityTest {
        /// 待测整数。
        n: Integer,
        /// Miller-Rabin 额外轮数（大整数）；`None` 用默认。
        miller_rabin_rounds: Option<u32>,
    },
    /// 整数因式分解。
    FactorInteger {
        /// 待分解整数。
        n: Integer,
        /// 资源上限。
        limits: FactorLimits,
    },
    /// 模逆 `a⁻¹ (mod m)`。
    ModInverse {
        /// 被逆元。
        a: Integer,
        /// 模数。
        modulus: Modulus,
    },
    /// 模幂 `base^exp mod m`（`exp ≥ 0`）。
    ModPow {
        /// 底。
        base: Integer,
        /// 非负指数。
        exp: Integer,
        /// 模数。
        modulus: Modulus,
    },
    /// 批量模逆（同一素模 / 互素剩余）。
    BatchModInverse {
        /// 剩余列表。
        residues: Vec<Integer>,
        /// 模数。
        modulus: Modulus,
    },
    /// 线性同余 `a x ≡ b (mod m)`。
    SolveLinearCongruence {
        /// `a`。
        a: Integer,
        /// `b`。
        b: Integer,
        /// 模 `m`（已验证）。
        modulus: Modulus,
    },
    /// 广义中国剩余定理：`x ≡ residues[i] (mod moduli[i])`。
    ChineseRemainder {
        /// 剩余。
        residues: Vec<Integer>,
        /// 模数（与 `residues` 等长）。
        moduli: Vec<Modulus>,
    },
    /// 有理重构：从 `residue (mod modulus)` 恢复 `n/d`。
    RationalReconstruction {
        /// 剩余。
        residue: Integer,
        /// 模数。
        modulus: Modulus,
        /// 分子绝对值上界；`None` → `⌊√(m/2)⌋`。
        max_numerator: Option<Integer>,
        /// 分母绝对值上界；`None` → `⌊√(m/2)⌋`。
        max_denominator: Option<Integer>,
    },
    /// 整数平方根 `⌊√n⌋`。
    Isqrt {
        /// 被开方数。
        n: Integer,
    },
    /// 完全幂分解。
    PerfectPower {
        /// 待检测整数。
        n: Integer,
    },
    /// Jacobi 符号 `(a/n)`。
    JacobiSymbol {
        /// 分子。
        a: Integer,
        /// 分母（正奇数）。
        n: Integer,
    },
    /// Kronecker 符号 `(a/n)`。
    KroneckerSymbol {
        /// 分子。
        a: Integer,
        /// 分母。
        n: Integer,
    },
    /// 筛法：不超过 `limit` 的全部素数。
    PrimesUpTo {
        /// 上界（含）。
        limit: u64,
    },
    /// 代数整数相关（骨架占位）。
    AlgebraicScaffold,
}

impl NumberTheoryRequest {
    /// Owning 复制（Living `31`：禁止默认 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Gcd { a, b } => Self::Gcd { a: clone_integer(a), b: clone_integer(b) },
            Self::Lcm { a, b } => Self::Lcm { a: clone_integer(a), b: clone_integer(b) },
            Self::ExtendedGcd { a, b } => Self::ExtendedGcd { a: clone_integer(a), b: clone_integer(b) },
            Self::PrimalityTest { n, miller_rabin_rounds } => {
                Self::PrimalityTest { n: clone_integer(n), miller_rabin_rounds: *miller_rabin_rounds }
            }
            Self::FactorInteger { n, limits } => Self::FactorInteger { n: clone_integer(n), limits: *limits },
            Self::ModInverse { a, modulus } => Self::ModInverse { a: clone_integer(a), modulus: clone_modulus(modulus) },
            Self::ModPow { base, exp, modulus } => {
                Self::ModPow { base: clone_integer(base), exp: clone_integer(exp), modulus: clone_modulus(modulus) }
            }
            Self::BatchModInverse { residues, modulus } => {
                Self::BatchModInverse { residues: clone_integers(residues), modulus: clone_modulus(modulus) }
            }
            Self::SolveLinearCongruence { a, b, modulus } => {
                Self::SolveLinearCongruence { a: clone_integer(a), b: clone_integer(b), modulus: clone_modulus(modulus) }
            }
            Self::ChineseRemainder { residues, moduli } => {
                Self::ChineseRemainder { residues: clone_integers(residues), moduli: moduli.iter().map(clone_modulus).collect() }
            }
            Self::RationalReconstruction { residue, modulus, max_numerator, max_denominator } => Self::RationalReconstruction {
                residue: clone_integer(residue),
                modulus: clone_modulus(modulus),
                max_numerator: max_numerator.as_ref().map(clone_integer),
                max_denominator: max_denominator.as_ref().map(clone_integer),
            },
            Self::Isqrt { n } => Self::Isqrt { n: clone_integer(n) },
            Self::PerfectPower { n } => Self::PerfectPower { n: clone_integer(n) },
            Self::JacobiSymbol { a, n } => Self::JacobiSymbol { a: clone_integer(a), n: clone_integer(n) },
            Self::KroneckerSymbol { a, n } => Self::KroneckerSymbol { a: clone_integer(a), n: clone_integer(n) },
            Self::PrimesUpTo { limit } => Self::PrimesUpTo { limit: *limit },
            Self::AlgebraicScaffold => Self::AlgebraicScaffold,
        }
    }
}
