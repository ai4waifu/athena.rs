//! 整数因式分解（试除 → rho → p−1 → ECM → QS）。

mod ecm;
mod p1;
mod policy;
mod producer;
mod qs;
mod rho;
mod verifier;

use athena_numeric::Integer;
use athena_types::Diagnostic;

use super::{
    certificates::PrimeCertificate,
    primes::primality_test,
    result::factor_zero_invalid,
    value::{CofactorStatus, FactorBaseStatus, FactorComponent, Factorization, Primality, factor_status_from_primality},
};

pub use policy::{FactorAlgorithms, FactorExecutionBudget, FactorFrontier, FactorLimits, FactorPolicy, ProofRequirement};
pub use producer::{FactorProducer, PureRustFactorProducer};
pub use qs::{dixon_split, fermat_split, qs_split};
pub use verifier::{FactorizationVerifyError, verify_factorization};

use ecm::ecm_stage_one;
use p1::pollard_p1;
use rho::pollard_rho;

/// 整数因式分解（试除 + rho → p−1 → ECM → QS）。
///
/// `0` 无有限素因数分解 → `Err`（域错误）。
pub fn factor_integer(n: &Integer, limits: &FactorLimits) -> Result<Factorization, Diagnostic> {
    factor_integer_with_producer(n, limits, &PureRustFactorProducer)
}

/// 带可插拔 producer 的分解入口（外部 GMP-ECM 等可选挂接）。
pub fn factor_integer_with_producer<P: FactorProducer>(
    n: &Integer,
    limits: &FactorLimits,
    producer: &P,
) -> Result<Factorization, Diagnostic> {
    if n.is_zero() {
        return Err(factor_zero_invalid());
    }

    let unit = if n.is_negative() { Integer::from_i64(-1) } else { Integer::one() };
    let m = n.abs();

    if m.is_one() {
        return Ok(complete_factorization(unit, Vec::new(), false));
    }

    if m.bits() > u64::from(limits.max_bits()) {
        return Ok(Factorization {
            unit,
            factors: Vec::new(),
            cofactor: m,
            cofactor_status: CofactorStatus::CompositeUnsplit,
            input_rejected: true,
            resource_exhausted: false,
        });
    }

    let mut frontier = FactorFrontier {
        unit,
        factors_found: Vec::new(),
        unresolved_cofactors: vec![m],
        steps_used: 0,
        resource_exhausted: false,
    };

    if limits.policy.algorithms.trial {
        let Some(remaining) = frontier.unresolved_cofactors.pop()
        else {
            return Ok(finalize_frontier(frontier));
        };
        let mut work = remaining;
        trial_division(&mut work, limits, &mut frontier.factors_found);
        if work > Integer::one() {
            frontier.unresolved_cofactors.push(work);
        }
    }

    run_composite_pipeline(&mut frontier, limits, producer);
    Ok(finalize_frontier(frontier))
}

/// 从已有 [`FactorFrontier`] 续算。
pub fn factor_continue(frontier: FactorFrontier, limits: &FactorLimits) -> Result<Factorization, Diagnostic> {
    factor_continue_with_producer(frontier, limits, &PureRustFactorProducer)
}

/// 带 producer 的续算。
pub fn factor_continue_with_producer<P: FactorProducer>(
    mut frontier: FactorFrontier,
    limits: &FactorLimits,
    producer: &P,
) -> Result<Factorization, Diagnostic> {
    frontier.resource_exhausted = false;
    run_composite_pipeline(&mut frontier, limits, producer);
    Ok(finalize_frontier(frontier))
}

fn finalize_frontier(frontier: FactorFrontier) -> Factorization {
    let FactorFrontier { unit, mut factors_found, unresolved_cofactors, resource_exhausted, .. } = frontier;

    let mut cofactor = Integer::one();
    let mut unknown = false;
    let mut composite = false;
    for c in unresolved_cofactors {
        if c.is_one() {
            continue;
        }
        let prim = primality_test(&c, None);
        match &prim {
            Primality::Prime { .. } | Primality::ProbablePrime { .. } => {
                let status = factor_status_from_primality(&prim).expect("prime");
                push_or_merge(&mut factors_found, c, status);
            }
            Primality::Composite { .. } => {
                composite = true;
                cofactor = cofactor.mul(&c);
            }
            Primality::Unknown => {
                unknown = true;
                cofactor = cofactor.mul(&c);
            }
        }
    }

    sort_factors(&mut factors_found);
    let cofactor_status = if cofactor.is_one() {
        CofactorStatus::One
    }
    else if unknown {
        CofactorStatus::Unknown
    }
    else if composite {
        CofactorStatus::CompositeUnsplit
    }
    else {
        CofactorStatus::Unknown
    };

    Factorization { unit, factors: factors_found, cofactor, cofactor_status, input_rejected: false, resource_exhausted }
}

fn complete_factorization(unit: Integer, mut factors: Vec<FactorComponent>, resource_exhausted: bool) -> Factorization {
    sort_factors(&mut factors);
    Factorization {
        unit,
        factors,
        cofactor: Integer::one(),
        cofactor_status: CofactorStatus::One,
        input_rejected: false,
        resource_exhausted,
    }
}

fn sort_factors(factors: &mut Vec<FactorComponent>) {
    factors.sort_by(|a, b| a.base.cmp(&b.base));
}

fn trial_division(m: &mut Integer, limits: &FactorLimits, factors: &mut Vec<FactorComponent>) {
    let trial_cert = PrimeCertificate::TrialDivision { bound: limits.max_trial() };
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
            status: FactorBaseStatus::ProvenPrime { certificate: PrimeCertificate::SmallPrime },
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
        }
        else if pb.mul(&pb) > *m && *m > Integer::one() {
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
                status: FactorBaseStatus::ProvenPrime { certificate: trial_cert.clone() },
            });
        }
        p = p.saturating_add(2);
        if p > trial_cap {
            break;
        }
    }
}

fn run_composite_pipeline<P: FactorProducer>(frontier: &mut FactorFrontier, limits: &FactorLimits, producer: &P) {
    let max_steps = limits.budget.max_steps.unwrap_or(500_000);
    let mut stack = std::mem::take(&mut frontier.unresolved_cofactors);

    while let Some(n) = stack.pop() {
        if n.is_one() {
            continue;
        }
        let prim = primality_test(&n, None);
        if matches!(prim, Primality::Prime { .. } | Primality::ProbablePrime { .. }) {
            let status = factor_status_from_primality(&prim).expect("prime");
            push_or_merge(&mut frontier.factors_found, n, status);
            continue;
        }
        if frontier.steps_used >= max_steps {
            frontier.resource_exhausted = true;
            frontier.unresolved_cofactors.push(n);
            frontier.unresolved_cofactors.extend(stack);
            return;
        }
        match try_split(&n, limits, &mut frontier.steps_used, max_steps, producer) {
            Some(d) if d > Integer::one() && d != n => {
                let q = n.div(&d);
                stack.push(d);
                stack.push(q);
            }
            _ => {
                // 本轮未能分裂：留下待续算。
                frontier.unresolved_cofactors.push(n);
            }
        }
    }
}

fn try_split<P: FactorProducer>(
    n: &Integer,
    limits: &FactorLimits,
    steps: &mut u64,
    max_steps: u64,
    producer: &P,
) -> Option<Integer> {
    if *steps >= max_steps {
        return None;
    }
    let seed = limits.policy.deterministic_seed.unwrap_or(1);
    let alg = limits.policy.algorithms;
    let b1 = limits.policy.stage1_b1;
    let curves = limits.policy.ecm_curves;
    let remain = max_steps.saturating_sub(*steps);

    if let Some(d) = producer.try_split(n, seed, remain) {
        *steps += 1;
        return Some(d);
    }

    if alg.pollard_rho {
        *steps += 1;
        if let Some(d) = pollard_rho(n, seed, 1, remain) {
            return Some(d);
        }
    }
    if alg.pollard_p1 {
        *steps += 1;
        if let Some(d) = pollard_p1(n, seed.wrapping_add(7), b1) {
            return Some(d);
        }
    }
    if alg.ecm {
        *steps += 1;
        if let Some(d) = ecm_stage_one(n, seed.wrapping_add(11), b1, curves) {
            return Some(d);
        }
    }
    if alg.quadratic_sieve {
        *steps += 1;
        if let Some(d) = qs_split(n, seed.wrapping_add(17), remain) {
            return Some(d);
        }
    }
    None
}

fn push_or_merge(factors: &mut Vec<FactorComponent>, base: Integer, status: FactorBaseStatus) {
    if let Some(slot) = factors.iter_mut().find(|c| c.base == base) {
        slot.exponent += 1;
    }
    else {
        factors.push(FactorComponent { base, exponent: 1, status });
    }
}

/// 由素性构造 [`FactorComponent`]（供测试 / 外部 producer）。
pub fn factor_component_from_primality(base: Integer, exponent: u32, primality: Primality) -> Option<FactorComponent> {
    factor_status_from_primality(&primality).map(|status| FactorComponent { base, exponent, status })
}

/// 将部分分解结果转为可续算前沿。
pub fn factorization_to_frontier(f: Factorization) -> FactorFrontier {
    let mut unresolved = Vec::new();
    if f.cofactor > Integer::one() {
        unresolved.push(f.cofactor);
    }
    FactorFrontier {
        unit: f.unit,
        factors_found: f.factors,
        unresolved_cofactors: unresolved,
        steps_used: 0,
        resource_exhausted: f.resource_exhausted,
    }
}
