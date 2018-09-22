//! 数论域请求。

use athena_numeric::{Integer, Modulus};

use super::factor::FactorLimits;

/// 数论域请求 — 宿主传入已解码整数 / 模数。
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// 扩展欧几里得。
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
    /// 线性同余 `a x ≡ b (mod m)`（骨架）。
    SolveLinearCongruence {
        /// `a`。
        a: Integer,
        /// `b`。
        b: Integer,
        /// 模 `m`。
        modulus: Integer,
    },
    /// 代数整数相关（骨架占位）。
    AlgebraicScaffold,
}
