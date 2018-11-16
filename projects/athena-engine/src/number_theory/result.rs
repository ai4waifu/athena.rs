//! 数论请求分派。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    algebraic::algebraic_scaffold,
    congruence::solve_linear_congruence,
    factor::factor_integer,
    gcd::{extended_gcd, gcd, lcm},
    modular::{mod_inverse, mod_pow},
    primes::primality_test,
    request::NumberTheoryRequest,
    value::{FactorizationCompleteness, NumberTheoryValue, Primality},
};

/// 数论域结果信封。
///
/// 外层确定性必须与内层对象一致：禁止把 `ProbablePrime` / `Partial` /
/// `ResourceLimited` 包装成数学意义上的 `Exact`。
#[derive(Debug, Clone, PartialEq)]
pub enum NumberTheoryResult {
    /// 数学结论确定（gcd、确定素数/合数、完全确定分解、成功模运算等）。
    Exact {
        /// 结果值。
        value: NumberTheoryValue,
    },
    /// 概率结论（概率素数、概率完全分解）。
    Probable {
        /// 结果值。
        value: NumberTheoryValue,
    },
    /// 部分结果（仍有未分解余因子等）。
    Partial {
        /// 结果值。
        value: NumberTheoryValue,
    },
    /// 触及资源上限；值可能仍部分可用。
    ResourceLimited {
        /// 结果值。
        value: NumberTheoryValue,
    },
    /// 算法未给出可用结论（例如素性 `Unknown`）。
    Inconclusive {
        /// 结果值（若有结构化占位）。
        value: NumberTheoryValue,
    },
    /// 域上不适用的输入（例如 `FactorInteger(0)`）。
    InvalidInput {
        /// 失败原因。
        reason: Diagnostic,
    },
    /// 算法未完成或前置失败（诊断码稳定）。
    Unevaluated {
        /// 失败原因。
        reason: Diagnostic,
    },
}

/// 执行数论域请求。
pub fn execute_number_theory(request: NumberTheoryRequest) -> NumberTheoryResult {
    match request {
        NumberTheoryRequest::Gcd { a, b } => NumberTheoryResult::Exact {
            value: NumberTheoryValue::Integer(gcd(&a, &b)),
        },
        NumberTheoryRequest::Lcm { a, b } => NumberTheoryResult::Exact {
            value: NumberTheoryValue::Integer(lcm(&a, &b)),
        },
        NumberTheoryRequest::ExtendedGcd { a, b } => NumberTheoryResult::Exact {
            value: NumberTheoryValue::ExtendedGcd(extended_gcd(&a, &b)),
        },
        NumberTheoryRequest::PrimalityTest { n, miller_rabin_rounds } => {
            wrap_primality(primality_test(&n, miller_rabin_rounds))
        }
        NumberTheoryRequest::FactorInteger { n, limits } => match factor_integer(&n, &limits) {
            Ok(f) => wrap_factorization(f),
            Err(reason) => NumberTheoryResult::InvalidInput { reason },
        },
        NumberTheoryRequest::ModInverse { a, modulus } => match mod_inverse(&a, &modulus) {
            Ok(v) => NumberTheoryResult::Exact {
                value: NumberTheoryValue::Modular(v),
            },
            Err(reason) => NumberTheoryResult::Unevaluated { reason },
        },
        NumberTheoryRequest::ModPow { base, exp, modulus } => match mod_pow(&base, &exp, &modulus) {
            Ok(v) => NumberTheoryResult::Exact {
                value: NumberTheoryValue::Modular(v),
            },
            Err(reason) => NumberTheoryResult::Unevaluated { reason },
        },
        NumberTheoryRequest::SolveLinearCongruence { a, b, modulus } => {
            solve_linear_congruence(&a, &b, &modulus)
        }
        NumberTheoryRequest::AlgebraicScaffold => algebraic_scaffold(),
    }
}

fn wrap_primality(p: Primality) -> NumberTheoryResult {
    let value = NumberTheoryValue::Primality(p.clone());
    match p {
        Primality::Prime | Primality::Composite => NumberTheoryResult::Exact { value },
        Primality::ProbablePrime { .. } => NumberTheoryResult::Probable { value },
        Primality::Unknown => NumberTheoryResult::Inconclusive { value },
    }
}

fn wrap_factorization(f: super::value::Factorization) -> NumberTheoryResult {
    let completeness = f.completeness;
    let value = NumberTheoryValue::Factorization(f);
    match completeness {
        FactorizationCompleteness::Complete => NumberTheoryResult::Exact { value },
        FactorizationCompleteness::Probable => NumberTheoryResult::Probable { value },
        FactorizationCompleteness::Partial => NumberTheoryResult::Partial { value },
        FactorizationCompleteness::ResourceLimited => NumberTheoryResult::ResourceLimited { value },
    }
}

/// `FactorInteger(0)` 等域错误构造。
pub(crate) fn factor_zero_invalid() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::DomainError)
        .detail("domain", "number_theory")
        .detail("operation", "factor_integer")
        .detail("reason", "zero_has_no_finite_prime_factorization")
}
