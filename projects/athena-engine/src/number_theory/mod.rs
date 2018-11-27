//! 数论领域 — gcd / 素性 / 分解 / 模运算 / 同余（Gate 1 + P1/P2）。
//!
//! 结果带完整性与确定性元数据；禁止把 probable 素性当成确定 `Prime`，
//! 禁止裸 `Vec` 让宿主猜测分解是否完整。

mod algebraic;
mod certificates;
mod congruence;
mod factor;
mod gcd;
mod modular;
mod primes;
mod request;
mod result;
mod value;

pub use certificates::{CompositeWitness, PrimeCertificate, ProbablePrimeEvidence};
pub use congruence::{chinese_remainder, chinese_remainder_pair, rational_reconstruction, solve_linear_congruence};
pub use factor::{
    FactorAlgorithms, FactorExecutionBudget, FactorFrontier, FactorLimits, FactorPolicy,
    FactorizationVerifyError, ProofRequirement, factor_component_from_primality, factor_integer,
    verify_factorization,
};
pub use gcd::{extended_gcd, gcd, lcm};
pub use modular::{mod_inverse, mod_pow};
pub use primes::primality_test;
pub use request::NumberTheoryRequest;
pub use result::{NumberTheoryResult, execute_number_theory};
pub use value::{
    CofactorStatus, CongruenceSolution, CrtResult, ExtendedGcd, FactorBaseStatus, FactorComponent,
    Factorization, FactorizationCompleteness, MillerRabinBaseSelection, NumberTheoryValue, Primality,
    RationalReconstruction, RationalReconstructionFailure,
};
