//! Dixon QS + Fermat QS 阶段测试。

use athena_engine::{
    FactorAlgorithms, FactorLimits, FactorizationCompleteness, Integer, dixon_split, factor_integer, fermat_split, qs_split,
    verify_factorization,
};

#[test]
fn fermat_still_splits_close_semiprime() {
    let n = Integer::from_i64(1_000_003).mul(&Integer::from_i64(1_000_033));
    let d = fermat_split(&n, 100_000).expect("fermat");
    assert!(d > Integer::one() && d < n);
    assert_eq!(n.rem(&d).expect("rem"), Integer::zero());
}

#[test]
fn dixon_splits_small_semiprime() {
    let n = Integer::from_i64(8051); // 83×97
    let d = dixon_split(&n, 1, 200_000).expect("dixon");
    assert!(d > Integer::one() && d < n);
    assert_eq!(n.rem(&d).expect("rem"), Integer::zero());
}

#[test]
fn qs_split_prefers_fermat_then_dixon() {
    let close = Integer::from_i64(1_000_003).mul(&Integer::from_i64(1_000_033));
    let d = qs_split(&close, 7, 100_000).expect("qs close");
    assert_eq!(close.rem(&d).expect("rem"), Integer::zero());

    let mid = Integer::from_i64(8051);
    let d2 = qs_split(&mid, 3, 200_000).expect("qs mid");
    assert_eq!(mid.rem(&d2).expect("rem"), Integer::zero());
}

#[test]
fn qs_pipeline_only_splits_semiprime() {
    let n = Integer::from_i64(8051);
    let mut limits = FactorLimits::default();
    limits.policy.algorithms =
        FactorAlgorithms { trial: false, pollard_rho: false, pollard_p1: false, ecm: false, quadratic_sieve: true };
    limits.budget.max_steps = Some(200_000);
    let f = factor_integer(&n, &limits).expect("factor");
    assert_eq!(f.completeness(), FactorizationCompleteness::Complete);
    verify_factorization(&n, &f).expect("verify");
}
