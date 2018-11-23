//! 整数因式分解（试除 bootstrap）。

mod policy;
mod verifier;

use athena_numeric::Integer;
use athena_types::Diagnostic;

use super::{
    certificates::PrimeCertificate,
    primes::primality_test,
    result::factor_zero_invalid,
    value::{
        CofactorStatus, FactorBaseStatus, FactorComponent, Factorization, Primality,
        factor_status_from_primality,
    },
};

pub use policy::{
    FactorAlgorithms, FactorExecutionBudget, FactorFrontier, FactorLimits, FactorPolicy, ProofRequirement,
};
pub use verifier::{FactorizationVerifyError, verify_factorization};

/// 整数因式分解（试除 bootstrap）。
///
/// `0` 无有限素因数分解 → `Err`（域错误）。
pub fn factor_integer(n: &Integer, limits: &FactorLimits) -> Result<Factorization, Diagnostic> {
    if n.is_zero() {
        return Err(factor_zero_invalid());
    }

    let unit = if n.is_negative() {
        Integer::from_i64(-1)
    } else {
        Integer::one()
    };
    let mut m = n.abs();

    if m.is_one() {
        return Ok(Factorization {
            unit,
            factors: Vec::new(),
            cofactor: Integer::one(),
            cofactor_status: CofactorStatus::One,
            input_rejected: false,
        });
    }

    if m.bits() > u64::from(limits.max_bits()) {
        return Ok(Factorization {
            unit,
            factors: Vec::new(),
            cofactor: m,
            cofactor_status: CofactorStatus::CompositeUnsplit,
            input_rejected: true,
        });
    }

    let mut factors: Vec<FactorComponent> = Vec::new();
    let trial_cert = PrimeCertificate::TrialDivision {
        bound: limits.max_trial(),
    };

    let two = Integer::from_i64(2);
    let mut exp2 = 0u32;
    while m.rem(&two).is_zero() {
        m = m.div(&two);
        exp2 += 1;
    }
    if exp2 > 0 {
        factors.push(FactorComponent {
            base: two,
            exponent: exp2,
            status: FactorBaseStatus::ProvenPrime {
                certificate: PrimeCertificate::SmallPrime,
            },
        });
    }

    let mut p: u64 = 3;
    let trial_cap = limits.max_trial();
    while p <= trial_cap {
        let pb = Integer::from_u64(p);
        if let (Some(ps), Some(ms)) = (pb.to_u128(), m.to_u128()) {
            if ps.saturating_mul(ps) > ms && m > Integer::one() {
                break;
            }
        } else if pb.mul(&pb) > m && m > Integer::one() {
            break;
        }

        let mut e = 0u32;
        while m.rem(&pb).is_zero() {
            m = m.div(&pb);
            e += 1;
        }
        if e > 0 {
            factors.push(FactorComponent {
                base: pb,
                exponent: e,
                status: FactorBaseStatus::ProvenPrime {
                    certificate: trial_cert.clone(),
                },
            });
        }
        p = p.saturating_add(2);
        if p > trial_cap {
            break;
        }
    }

    if m.is_one() {
        return Ok(Factorization {
            unit,
            factors,
            cofactor: Integer::one(),
            cofactor_status: CofactorStatus::One,
            input_rejected: false,
        });
    }

    let prim = primality_test(&m, None);
    match &prim {
        Primality::Prime { .. } | Primality::ProbablePrime { .. } => {
            let status = factor_status_from_primality(&prim).expect("prime or probable");
            factors.push(FactorComponent {
                base: m,
                exponent: 1,
                status,
            });
            Ok(Factorization {
                unit,
                factors,
                cofactor: Integer::one(),
                cofactor_status: CofactorStatus::One,
                input_rejected: false,
            })
        }
        Primality::Composite { .. } | Primality::Unknown => {
            let cofactor_status = if matches!(prim, Primality::Unknown) {
                CofactorStatus::Unknown
            } else {
                CofactorStatus::CompositeUnsplit
            };
            Ok(Factorization {
                unit,
                factors,
                cofactor: m,
                cofactor_status,
                input_rejected: false,
            })
        }
    }
}

/// 由素性构造 [`FactorComponent`]（供测试 / 外部 producer）。
pub fn factor_component_from_primality(
    base: Integer,
    exponent: u32,
    primality: Primality,
) -> Option<FactorComponent> {
    factor_status_from_primality(&primality).map(|status| FactorComponent {
        base,
        exponent,
        status,
    })
}
