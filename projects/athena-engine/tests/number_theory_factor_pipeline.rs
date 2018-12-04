//! 分解 pipeline（rho → p−1 → ECM → QS）与可续算 frontier。

use athena_engine::{
    FactorAlgorithms, FactorLimits, FactorizationCompleteness, Integer, factor_continue, factor_integer,
    factorization_to_frontier, verify_factorization,
};

#[test]
fn pollard_p1_splits_smooth_order_semiprime() {
    // 47−1 = 46 = 2·23（B1≥23）；104729−1 含大素因子，仅裂出 47
    let n = Integer::from_i64(47).mul(&Integer::from_i64(104729));
    let mut limits = FactorLimits::default();
    limits.policy.algorithms = FactorAlgorithms {
        trial: false,
        pollard_rho: false,
        pollard_p1: true,
        ecm: false,
        quadratic_sieve: false,
    };
    limits.policy.stage1_b1 = 30;
    limits.budget.max_trial = 0;
    limits.budget.max_steps = Some(100);
    let f = factor_integer(&n, &limits).expect("factor");
    assert_eq!(f.completeness(), FactorizationCompleteness::Complete);
    verify_factorization(&n, &f).expect("verify");
    assert_eq!(f.factors.len(), 2);
}

#[test]
fn pipeline_factors_semiprime() {
    let n = Integer::from_i64(10403).mul(&Integer::from_i64(104729));
    let mut limits = FactorLimits::default();
    limits.policy.algorithms = FactorAlgorithms::with_pipeline();
    limits.budget.max_trial = 100;
    limits.budget.max_steps = Some(500_000);
    let f = factor_integer(&n, &limits).expect("factor");
    assert_eq!(f.completeness(), FactorizationCompleteness::Complete);
    verify_factorization(&n, &f).expect("verify");
}

#[test]
fn resource_budget_marks_resource_limited_then_continue() {
    // 因子间距大，Fermat 需大量步数；max_steps=0 立即 ResourceLimited
    let n = Integer::from_i64(10403).mul(&Integer::from_i64(104729));
    let mut tight = FactorLimits::default();
    tight.policy.algorithms = FactorAlgorithms {
        trial: false,
        pollard_rho: false,
        pollard_p1: false,
        ecm: false,
        quadratic_sieve: true,
    };
    tight.budget.max_steps = Some(0);
    let partial = factor_integer(&n, &tight).expect("partial");
    assert_eq!(partial.completeness(), FactorizationCompleteness::ResourceLimited);
    assert!(partial.resource_exhausted);

    let frontier = factorization_to_frontier(partial);
    let mut loose = tight.clone();
    loose.policy.algorithms = FactorAlgorithms::with_pipeline();
    loose.budget.max_trial = 100;
    loose.budget.max_steps = Some(500_000);
    let done = factor_continue(frontier, &loose).expect("continue");
    assert_eq!(done.completeness(), FactorizationCompleteness::Complete);
    verify_factorization(&n, &done).expect("verify continued");
}

#[test]
fn input_bits_rejection_is_resource_limited() {
    let n = Integer::from_i64(104729).mul(&Integer::from_i64(104729));
    let mut limits = FactorLimits::default();
    limits.budget.max_input_bits = 8;
    let f = factor_integer(&n, &limits).expect("reject");
    assert!(f.input_rejected);
    assert_eq!(f.completeness(), FactorizationCompleteness::ResourceLimited);
}
