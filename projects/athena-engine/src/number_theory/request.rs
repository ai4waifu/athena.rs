//! 数论域请求。

use num_bigint::BigInt;

use athena_types::Modulus;

use super::factor::FactorLimits;

/// 数论域请求 — 宿主传入已解码整数 / 模数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberTheoryRequest {
    /// `gcd(a, b)`（非负）。
    Gcd {
        /// 左操作数。
        a: BigInt,
        /// 右操作数。
        b: BigInt,
    },
    /// `lcm(a, b)`（非负；`0` 约定 `lcm(0,0)=0`）。
    Lcm {
        /// 左操作数。
        a: BigInt,
        /// 右操作数。
        b: BigInt,
    },
    /// 扩展欧几里得。
    ExtendedGcd {
        /// `a`。
        a: BigInt,
        /// `b`。
        b: BigInt,
    },
    /// 素性测试。
    PrimalityTest {
        /// 待测整数。
        n: BigInt,
        /// Miller-Rabin 额外轮数（大整数）；`None` 用默认。
        miller_rabin_rounds: Option<u32>,
    },
    /// 整数因式分解。
    FactorInteger {
        /// 待分解整数。
        n: BigInt,
        /// 资源上限。
        limits: FactorLimits,
    },
    /// 模逆 `a⁻¹ (mod m)`。
    ModInverse {
        /// 被逆元。
        a: BigInt,
        /// 模数。
        modulus: Modulus,
    },
    /// 模幂 `base^exp mod m`（`exp ≥ 0`）。
    ModPow {
        /// 底。
        base: BigInt,
        /// 非负指数。
        exp: BigInt,
        /// 模数。
        modulus: Modulus,
    },
}
