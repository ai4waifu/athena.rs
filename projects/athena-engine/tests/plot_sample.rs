//! 1D 采样骨架测试。

use athena_engine::{SampleDomain, SamplingPolicy, Term, evaluate, sample_1d};

#[test]
fn sample_square_on_unit_interval() {
    let expr = Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]);
    let curve = sample_1d(
        &expr,
        "x",
        SampleDomain::new(-1.0, 1.0),
        SamplingPolicy { max_samples: 5 },
    )
    .expect("sample");
    assert_eq!(curve.points.len(), 5);
    assert!(curve.gaps.is_empty());
    assert!((curve.points[0].y - 1.0).abs() < 1e-9);
    assert!((curve.points[2].y).abs() < 1e-9);
    assert!((curve.points[4].y - 1.0).abs() < 1e-9);
}

#[test]
fn sample_sin_has_finite_points() {
    let expr = Term::apply("Sin", vec![Term::symbol("x")]);
    let curve = sample_1d(
        &expr,
        "x",
        SampleDomain::new(0.0, std::f64::consts::PI),
        SamplingPolicy { max_samples: 17 },
    )
    .expect("sample");
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
    let err = sample_1d(&expr, "x", SampleDomain::new(0.0, 1.0), SamplingPolicy { max_samples: 1 }).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_SAMPLING_RESOURCE_LIMIT");
}

#[test]
fn reciprocal_marks_gap_at_zero() {
    let expr = Term::apply("Power", vec![Term::symbol("x"), Term::int(-1)]);
    let curve = sample_1d(
        &expr,
        "x",
        SampleDomain::new(-1.0, 1.0),
        SamplingPolicy { max_samples: 3 },
    )
    .expect("sample");
    assert!(!curve.points[1].valid);
    assert!(curve.gaps.contains(&1));
}
