//! 整数因式分解（试除 + rho / ECM / QS pipeline）。

mod ecm;
mod policy;
mod qs;
mod rho;
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

use ecm::ecm_stage_one;
use qs::fermat_split;
use rho::pollard_rho;

/// 整数因式分解（试除 + 可选 rho → ECM → QS）。
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
    let mut steps = 0u64;
    let max_steps = limits.budget.max_steps.unwrap_or(500_000);

    if limits.policy.algorithms.trial {
        trial_division(&mut m, limits, &mut factors);
    }

    if m.is_one() {
        return Ok(complete_factorization(unit, factors));
    }

    factor_composite_stack(&mut m, limits, &mut factors, &mut steps, max_steps)?;

    if m.is_one() {
        return Ok(complete_factorization(unit, factors));
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
            Ok(complete_factorization(unit, factors))
        }
        Primality::Composite { .. } | Primality::Unknown => {
            let cofactor_status = if matches!(prim, Primality::Unknown) {
                CofactorStatus::Unknown
            } else {
                CofactorStatus::CompositeUnsplit
            };
            Ok(Factorization {
                unit,
                factors: {
                    let mut fs = factors;
                    sort_factors(&mut fs);
                    fs
                },
                cofactor: m,
                cofactor_status,
                input_rejected: false,
            })
        }
    }
}

fn complete_factorization(unit: Integer, mut factors: Vec<FactorComponent>) -> Factorization {
    sort_factors(&mut factors);
    Factorization {
        unit,
        factors,
        cofactor: Integer::one(),
        cofactor_status: CofactorStatus::One,
        input_rejected: false,
    }
}

fn sort_factors(factors: &mut Vec<FactorComponent>) {
    factors.sort_by(|a, b| a.base.cmp(&b.base));
}

fn trial_division(m: &mut Integer, limits: &FactorLimits, factors: &mut Vec<FactorComponent>) {
    let trial_cert = PrimeCertificate::TrialDivision {
        bound: limits.max_trial(),
    };
    let two = Integer::from_i64(2);
    let mut exp2 = 0u32;
    while m.rem(&two).is_zero() {
        *m = m.div(&two);
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
            if ps.saturating_mul(ps) > ms && *m > Integer::one() {
                break;
            }
        } else if pb.mul(&pb) > *m && *m > Integer::one() {
            break;
        }

        let mut e = 0u32;
        while m.rem(&pb).is_zero() {
            *m = m.div(&pb);
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
}

fn factor_composite_stack(
    m: &mut Integer,
    limits: &FactorLimits,
    factors: &mut Vec<FactorComponent>,
    steps: &mut u64,
    max_steps: u64,
) -> Result<(), Diagnostic> {
    let mut stack = vec![m.clone()];
    while let Some(n) = stack.pop() {
        if n.is_one() {
            continue;
        }
        let prim = primality_test(&n, None);
        if matches!(prim, Primality::Prime { .. } | Primality::ProbablePrime { .. }) {
            let status = factor_status_from_primality(&prim).expect("prime");
            push_or_merge(factors, n, status);
            continue;
        }
        if let Some(d) = try_split(&n, limits, steps, max_steps) {
            if d.is_one() || d == n {
                stack.push(n);
                continue;
            }
            let q = n.div(&d);
            stack.push(d);
            stack.push(q);
        } else {
            *m = n;
            return Ok(());
        }
    }
    *m = Integer::one();
    Ok(())
}

fn try_split(n: &Integer, limits: &FactorLimits, steps: &mut u64, max_steps: u64) -> Option<Integer> {
    if *steps >= max_steps {
        return None;
    }
    let seed = limits.policy.deterministic_seed.unwrap_or(1);
    let alg = limits.policy.algorithms;

    if alg.pollard_rho {
        *steps += 1;
        if let Some(d) = pollard_rho(n, seed, 1, max_steps.saturating_sub(*steps)) {
            return Some(d);
        }
    }
    if alg.ecm {
        *steps += 1;
        if let Some(d) = ecm_stage_one(n, seed.wrapping_add(11), 200, 8) {
            return Some(d);
        }
    }
    if alg.quadratic_sieve {
        *steps += 1;
        if let Some(d) = fermat_split(n, max_steps.saturating_sub(*steps)) {
            return Some(d);
        }
    }
    None
}

fn push_or_merge(factors: &mut Vec<FactorComponent>, base: Integer, status: FactorBaseStatus) {
    if let Some(slot) = factors.iter_mut().find(|c| c.base == base) {
        slot.exponent += 1;
    } else {
        factors.push(FactorComponent {
            base,
            exponent: 1,
            status,
        });
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
