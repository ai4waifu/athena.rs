//! 数论领域 — gcd / 素性 / 分解 / 模运算（第一阶段 bootstrap）。
//!
//! 结果带完整性与确定性元数据；禁止把 probable 素性当成确定 `Prime`，
//! 禁止裸 `Vec` 让宿主猜测分解是否完整。

mod algebraic;
mod congruence;
mod factor;
mod gcd;
mod modular;
mod primes;
mod request;
mod result;
mod value;

pub use factor::{FactorLimits, factor_integer};
pub use gcd::{extended_gcd, gcd, lcm};
pub use modular::{mod_inverse, mod_pow};
pub use primes::primality_test;
pub use request::NumberTheoryRequest;
pub use result::{NumberTheoryResult, execute_number_theory};
pub use value::{ExtendedGcd, Factorization, FactorizationCompleteness, NumberTheoryValue, Primality, PrimePower};
