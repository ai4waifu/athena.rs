//! 数论请求分派。

use athena_types::Diagnostic;

use super::{
    factor::factor_integer,
    gcd::{extended_gcd, gcd, lcm},
    modular::{mod_inverse, mod_pow},
    primes::primality_test,
    request::NumberTheoryRequest,
    value::NumberTheoryValue,
};

/// 数论域结果（精确或未求值）。
#[derive(Debug, Clone, PartialEq)]
pub enum NumberTheoryResult {
    /// 成功求出带元数据的值。
    Exact {
        /// 结果值。
        value: NumberTheoryValue,
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
        NumberTheoryRequest::PrimalityTest { n, miller_rabin_rounds } => NumberTheoryResult::Exact {
            value: NumberTheoryValue::Primality(primality_test(&n, miller_rabin_rounds)),
        },
        NumberTheoryRequest::FactorInteger { n, limits } => NumberTheoryResult::Exact {
            value: NumberTheoryValue::Factorization(factor_integer(&n, &limits)),
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
    }
}
