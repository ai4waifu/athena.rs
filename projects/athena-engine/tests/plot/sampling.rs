//! 1D 采样合同测试（KernelIR 路径 · Living `25`）。

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use athena_engine::{
    plot::{SampleDomain, SamplingPolicy, sample_1d},
    runtime::{
        Session,
        values::arena::{push_application_named, push_int, push_symbol_name},
    },
};

type Tid = athena_types::TermId;

fn symbol(name: &str, s: &mut Session) -> Tid {
    push_symbol_name(s, name)
}

fn int(n: i64, s: &mut Session) -> Tid {
    push_int(s, n)
}

fn apply(head: &str, args: Vec<Tid>, s: &mut Session) -> Tid {
    push_application_named(s, head, args)
}

#[test]
fn sample_square_on_unit_interval() {
    let mut s = Session::new();
    let expr = apply("Power", vec![symbol("x", &mut s), int(2, &mut s)], &mut s);
    let curve = sample_1d(&mut s, expr, "x", SampleDomain::new(-1.0, 1.0), SamplingPolicy::samples(5)).expect("sample");
    assert_eq!(curve.points.len(), 5);
    assert!(curve.gaps.is_empty());
    assert!((curve.points[0].y - 1.0).abs() < 1e-9);
    assert!((curve.points[2].y).abs() < 1e-9);
    assert!((curve.points[4].y - 1.0).abs() < 1e-9);
}

#[test]
fn sample_sin_has_finite_points() {
    let mut s = Session::new();
    let expr = apply("Sin", vec![symbol("x", &mut s)], &mut s);
    let curve = sample_1d(&mut s, expr, "x", SampleDomain::new(0.0, std::f64::consts::PI), SamplingPolicy::samples(17)).expect("sample");
    let valid = curve.points.iter().filter(|p| p.valid).count();
    assert!(valid >= 15);
    assert!((curve.points[0].y).abs() < 1e-9);
    assert!((curve.points[8].y - 1.0).abs() < 1e-6);
}

#[test]
fn invalid_domain_and_policy() {
    let mut s = Session::new();
    let expr = symbol("x", &mut s);
    let err = sample_1d(&mut s, expr, "x", SampleDomain::new(1.0, 0.0), SamplingPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_SAMPLING_DOMAIN_INVALID");
    let err = sample_1d(&mut s, expr, "x", SampleDomain::new(0.0, 1.0), SamplingPolicy::samples(1)).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_SAMPLING_RESOURCE_LIMIT");
}

#[test]
fn reciprocal_marks_gap_at_zero() {
    let mut s = Session::new();
    let expr = apply("Power", vec![symbol("x", &mut s), int(-1, &mut s)], &mut s);
    let curve = sample_1d(&mut s, expr, "x", SampleDomain::new(-1.0, 1.0), SamplingPolicy::samples(3)).expect("sample");
    assert!(!curve.points[1].valid);
    assert!(curve.gaps.contains(&1));
}

#[test]
fn cancel_aborts_sampling() {
    let mut s = Session::new();
    let flag = Arc::new(AtomicBool::new(true));
    let expr = symbol("x", &mut s);
    let policy = SamplingPolicy { max_samples: 8, discontinuity_rel: None, cancel: Some(flag) };
    let err = sample_1d(&mut s, expr, "x", SampleDomain::new(0.0, 1.0), policy).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_SAMPLING_CANCELLED");
}

#[test]
fn mid_loop_cancel() {
    let mut s = Session::new();
    let flag = Arc::new(AtomicBool::new(false));
    let flag2 = Arc::clone(&flag);
    // 未取消时成功，取消后报 SamplingCancelled。
    let expr = apply("Power", vec![symbol("x", &mut s), int(2, &mut s)], &mut s);
    let ok = sample_1d(
        &mut s,
        expr,
        "x",
        SampleDomain::new(0.0, 1.0),
        SamplingPolicy { max_samples: 4, discontinuity_rel: None, cancel: Some(Arc::clone(&flag2)) },
    );
    assert!(ok.is_ok());
    flag.store(true, Ordering::Relaxed);
    let err = sample_1d(
        &mut s,
        expr,
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
    let mut s = Session::new();
    let expr = apply("Tan", vec![symbol("x", &mut s)], &mut s);
    let curve = sample_1d(
        &mut s,
        expr,
        "x",
        SampleDomain::new(1.0, 2.0),
        SamplingPolicy { max_samples: 9, discontinuity_rel: Some(1.0), cancel: None },
    )
    .expect("sample");
    assert!(!curve.gaps.is_empty() || curve.points.iter().any(|p| !p.valid), "curve={curve:?}");
}
