//! 数论领域 — gcd / 素性 / 分解 / 模运算 / 同余（Gate 1 + P1/P2）。
//!
//! 结果带完整性与确定性元数据；禁止把 probable 素性当成确定 `Prime`，
//! 禁止裸 `Vec` 让宿主猜测分解是否完整。

mod algebraic;
mod arithmetic;
mod certificates;
mod congruence;
mod factor;
mod gcd;
mod modular;
mod primes;
mod request;
mod result;
mod value;

pub use arithmetic::{is_perfect_power, isqrt, isqrt_if_exact, jacobi_symbol, kronecker_symbol, perfect_power_decomposition};
pub use certificates::{CompositeWitness, PrimeCertificate, ProbablePrimeEvidence};
pub use congruence::{chinese_remainder, chinese_remainder_pair, rational_reconstruction, solve_linear_congruence};
pub use factor::{
    FactorAlgorithms, FactorExecutionBudget, FactorFrontier, FactorLimits, FactorPolicy, FactorProducer,
    FactorizationVerifyError, ProofRequirement, PureRustFactorProducer, factor_component_from_primality, factor_continue,
    factor_continue_with_producer, factor_integer, factor_integer_with_producer, factorization_to_frontier,
    verify_factorization,
};
pub use gcd::{extended_gcd, gcd, lcm};
pub use modular::{batch_mod_inverse, mod_inverse, mod_inverse_with_table, mod_pow, mod_pow_with_table};
pub use primes::{PrimeIterator, next_prime_after, primality_test, primes_up_to};
pub use request::NumberTheoryRequest;
pub use result::{NumberTheoryResult, execute_number_theory};
pub use value::{
    CofactorStatus, CongruenceSolution, CrtResult, ExtendedGcd, FactorBaseStatus, FactorComponent, Factorization,
    FactorizationCompleteness, MillerRabinBaseSelection, NumberTheoryValue, Primality, RationalReconstruction,
    RationalReconstructionFailure,
};
