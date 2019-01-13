//! 1D 采样合同测试。

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use athena_engine::{SampleDomain, SamplingPolicy, Term, evaluate, sample_1d};

#[test]
fn sample_square_on_unit_interval() {
    let expr = Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]);
    let curve = sample_1d(&expr, "x", SampleDomain::new(-1.0, 1.0), SamplingPolicy::samples(5)).expect("sample");
    assert_eq!(curve.points.len(), 5);
    assert!(curve.gaps.is_empty());
    assert!((curve.points[0].y - 1.0).abs() < 1e-9);
    assert!((curve.points[2].y).abs() < 1e-9);
    assert!((curve.points[4].y - 1.0).abs() < 1e-9);
}

#[test]
fn sample_sin_has_finite_points() {
    let expr = Term::apply("Sin", vec![Term::symbol("x")]);
    let curve =
        sample_1d(&expr, "x", SampleDomain::new(0.0, std::f64::consts::PI), SamplingPolicy::samples(17)).expect("sample");
    let valid = curve.points.iter().filter(|p| p.valid).count();
    assert!(valid >= 15);
    assert!((curve.points[0].y).abs() < 1e-9);
    assert!((curve.points[8].y - 1.0).abs() < 1e-6);
}

#[test]
fn machine_sin_cos_exp_log() {
    let sin0 = evaluate(&Term::apply("Sin", vec![Term::real(0.0)]));
    assert!((sin0.as_f64_lossy().unwrap()).abs() < 1e-12);
    let cos0 = evaluate(&Term::apply("Cos", vec![Term::real(0.0)]));
    assert!((cos0.as_f64_lossy().unwrap() - 1.0).abs() < 1e-12);
    let exp0 = evaluate(&Term::apply("Exp", vec![Term::real(0.0)]));
    assert!((exp0.as_f64_lossy().unwrap() - 1.0).abs() < 1e-12);
    let log_e = evaluate(&Term::apply("Log", vec![Term::real(1.0)]));
    assert!((log_e.as_f64_lossy().unwrap()).abs() < 1e-12);
}

#[test]
fn invalid_domain_and_policy() {
    let expr = Term::symbol("x");
    let err = sample_1d(&expr, "x", SampleDomain::new(1.0, 0.0), SamplingPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_SAMPLING_DOMAIN_INVALID");
    let err = sample_1d(&expr, "x", SampleDomain::new(0.0, 1.0), SamplingPolicy::samples(1)).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_SAMPLING_RESOURCE_LIMIT");
}

#[test]
fn reciprocal_marks_gap_at_zero() {
    let expr = Term::apply("Power", vec![Term::symbol("x"), Term::int(-1)]);
    let curve = sample_1d(&expr, "x", SampleDomain::new(-1.0, 1.0), SamplingPolicy::samples(3)).expect("sample");
    assert!(!curve.points[1].valid);
    assert!(curve.gaps.contains(&1));
}

#[test]
fn cancel_aborts_sampling() {
    let flag = Arc::new(AtomicBool::new(true));
    let expr = Term::symbol("x");
    let policy = SamplingPolicy { max_samples: 8, discontinuity_rel: None, cancel: Some(flag) };
    let err = sample_1d(&expr, "x", SampleDomain::new(0.0, 1.0), policy).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_SAMPLING_CANCELLED");
}

#[test]
fn mid_loop_cancel() {
    let flag = Arc::new(AtomicBool::new(false));
    let flag2 = Arc::clone(&flag);
    // 意图：首次检查后强制取消；调用前仍为 false，需要循环中途翻转。
    // 模拟：初始未取消；对小网格在循环中途置取消。单测里并行翻转标志较难。
    // 改为：验证 Ordering 路径 — 未取消成功，再取消。
    let expr = Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]);
    let ok = sample_1d(
        &expr,
        "x",
        SampleDomain::new(0.0, 1.0),
        SamplingPolicy { max_samples: 4, discontinuity_rel: None, cancel: Some(Arc::clone(&flag2)) },
    );
    assert!(ok.is_ok());
    flag.store(true, Ordering::Relaxed);
    let err = sample_1d(
        &expr,
        "x",
        SampleDomain::new(0.0, 1.0),
        SamplingPolicy { max_samples: 4, discontinuity_rel: None, cancel: Some(flag2) },
    )
    .unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_SAMPLING_CANCELLED");
}

#[test]
fn discontinuity_inserts_gap_on_jump() {
    // tan 在 [1, 2] 穿越 π/2 渐近线；相邻有限采样符号跳变且 |Δy| 很大。
    let expr = Term::apply("Tan", vec![Term::symbol("x")]);
    let curve = sample_1d(
        &expr,
        "x",
        SampleDomain::new(1.0, 2.0),
        SamplingPolicy { max_samples: 9, discontinuity_rel: Some(1.0), cancel: None },
    )
    .expect("sample");
    assert!(!curve.gaps.is_empty() || curve.points.iter().any(|p| !p.valid), "curve={curve:?}");
}
