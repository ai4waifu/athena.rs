//! 可移植算法性质 / 跨能力检查。
//!
//! 相对小学乘法或 Knuth 基线演练 planner 选定路径，
//! 且不依赖 kernel 私有辅助。

use athena_gc::{GcHeap, HeapBudget};
use athena_numeric::{
    CapabilityBundle, ExecutionBudget, NumericContext,
    algorithm::{DIV_BZ_THRESHOLD, MUL_KARATSUBA_THRESHOLD, MUL_TOOM_THRESHOLD, MulStrategy},
    natural::Natural,
};
use std::str::FromStr;

fn limbs_repeating(digit: &str, reps: usize) -> Natural {
    Natural::from_str(&digit.repeat(reps)).expect("parse")
}

#[test]
fn karatsuba_capability_matches_schoolbook_baseline() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let mut caps_full = CapabilityBundle::portable_default();
    caps_full.algorithm.toom = false;
    let ctx_k = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap.clone(), caps_full);

    let mut caps_sb = CapabilityBundle::portable_default();
    caps_sb.algorithm.karatsuba = false;
    caps_sb.algorithm.toom = false;
    let ctx_sb = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap, caps_sb);

    // Width above Karatsuba threshold, below typical Toom comfort if toom disabled.
    let reps = (MUL_KARATSUBA_THRESHOLD * 20).max(80);
    let a = limbs_repeating("123456789", reps);
    let b = limbs_repeating("987654321", reps);
    assert!(a.as_limbs().len() >= MUL_KARATSUBA_THRESHOLD);

    let plan = ctx_k.planner().plan_mul(a.as_limbs().len(), b.as_limbs().len());
    assert_eq!(plan, MulStrategy::Karatsuba);

    let pk = a.try_mul(&b, &ctx_k).expect("karatsuba mul");
    let ps = a.try_mul(&b, &ctx_sb).expect("schoolbook mul");
    assert_eq!(pk.as_limbs(), ps.as_limbs());
}

#[test]
fn toom_capability_matches_schoolbook_baseline() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx_t = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap.clone(), CapabilityBundle::portable_default());

    let mut caps_sb = CapabilityBundle::portable_default();
    caps_sb.algorithm.karatsuba = false;
    caps_sb.algorithm.toom = false;
    let ctx_sb = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap, caps_sb);

    let reps = (MUL_TOOM_THRESHOLD * 25).max(120);
    let a = limbs_repeating("314159265", reps);
    let b = limbs_repeating("271828182", reps);
    assert!(a.as_limbs().len().max(b.as_limbs().len()) >= MUL_TOOM_THRESHOLD);
    assert_eq!(ctx_t.planner().plan_mul(a.as_limbs().len(), b.as_limbs().len()), MulStrategy::Toom3);

    let pt = a.try_mul(&b, &ctx_t).expect("toom mul");
    let ps = a.try_mul(&b, &ctx_sb).expect("schoolbook mul");
    assert_eq!(pt.as_limbs(), ps.as_limbs());
}

#[test]
fn burnikel_ziegler_capability_matches_knuth_baseline() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx_bz = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap.clone(), CapabilityBundle::portable_default());

    let mut caps_kn = CapabilityBundle::portable_default();
    caps_kn.algorithm.bz_division = false;
    let ctx_kn = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap, caps_kn);

    let v = limbs_repeating("1000000007", 40);
    let q_hint = limbs_repeating("99991", 90);
    let u = q_hint.mul(&v).add(&Natural::from_u64(12345));
    assert!(u.as_limbs().len() >= DIV_BZ_THRESHOLD || u.as_limbs().len() >= 2 * v.as_limbs().len());

    let (q1, r1) = u.try_div_rem(&v, &ctx_bz).expect("bz");
    let (q2, r2) = u.try_div_rem(&v, &ctx_kn).expect("knuth");
    assert_eq!(q1.as_limbs(), q2.as_limbs());
    assert_eq!(r1.as_limbs(), r2.as_limbs());
    assert_eq!(q1.mul(&v).add(&r1), u);
}

#[test]
fn gcd_lehmer_path_matches_binary_reference_on_wide_odds() {
    // Wide odd operands: Lehmer loop engages then finishes via binary_gcd.
    let a = limbs_repeating("1357913579", 60);
    let b = limbs_repeating("2468024681", 55);
    let g = a.gcd(&b);

    // Euclidean reference via repeated rem on Naturals (same public API).
    let mut x = a.try_clone_in(&NumericContext::portable_default()).unwrap();
    let mut y = b.try_clone_in(&NumericContext::portable_default()).unwrap();
    while !y.is_zero() {
        let (_q, r) = x.div_rem(&y);
        x = y;
        y = r;
    }
    assert_eq!(g.as_limbs(), x.as_limbs());
}

#[test]
fn half_gcd_capability_matches_lehmer_baseline_on_wide_odds() {
    use athena_gc::{GcHeap, HeapBudget};
    use athena_numeric::algorithm::{GCD_HALF_THRESHOLD, GcdStrategy};

    let a = limbs_repeating("1357913579", 40);
    let b = limbs_repeating("2468024681", 36);
    assert!(a.as_limbs().len() >= GCD_HALF_THRESHOLD);
    assert!(b.as_limbs().len() >= GCD_HALF_THRESHOLD);

    let heap = GcHeap::new_shared(HeapBudget::default());
    let mut caps_h = CapabilityBundle::portable_default();
    caps_h.algorithm.half_gcd = true;
    let mut caps_l = CapabilityBundle::portable_default();
    caps_l.algorithm.half_gcd = false;

    assert_eq!(athena_numeric::algorithm::AlgorithmPlanner::new(caps_h).plan_gcd(a.as_limbs().len(), b.as_limbs().len()), GcdStrategy::HalfGcd);
    assert_eq!(athena_numeric::algorithm::AlgorithmPlanner::new(caps_l).plan_gcd(a.as_limbs().len(), b.as_limbs().len()), GcdStrategy::Lehmer);

    let ctx_h = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap.clone(), caps_h);
    let ctx_l = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap, caps_l);
    let g_h = a.try_gcd(&b, &ctx_h).expect("half");
    let g_l = a.try_gcd(&b, &ctx_l).expect("lehmer");
    assert_eq!(g_h.as_limbs(), g_l.as_limbs());
}

#[test]
fn half_gcd_planner_threshold_boundary() {
    use athena_numeric::algorithm::{AlgorithmPlanner, GCD_HALF_THRESHOLD, GcdStrategy};

    let mut caps = CapabilityBundle::portable_default();
    caps.algorithm.half_gcd = true;
    let p = AlgorithmPlanner::new(caps);
    assert_eq!(p.plan_gcd(GCD_HALF_THRESHOLD - 1, GCD_HALF_THRESHOLD - 1), GcdStrategy::Lehmer);
    assert_eq!(p.plan_gcd(GCD_HALF_THRESHOLD, GCD_HALF_THRESHOLD), GcdStrategy::HalfGcd);
}

#[test]
fn montgomery_mod_pow_matches_binary_mul_chain_on_odd_modulus() {
    let base = Natural::from_u64(3);
    let exp = Natural::from_u64(97);
    let modulus = Natural::from_str("1000000007").unwrap(); // odd prime
    let via = base.mod_pow(&exp, &modulus);

    let mut acc = Natural::from_u64(1);
    let mut b = base.try_clone_in(&NumericContext::portable_default()).unwrap();
    let mut e = exp.try_clone_in(&NumericContext::portable_default()).unwrap();
    while !e.is_zero() {
        if e.as_limbs()[0] & 1 == 1 {
            acc = acc.mul(&b).div_rem(&modulus).1;
        }
        b = b.mul(&b).div_rem(&modulus).1;
        // e >>= 1 via div 2
        e = e.div_rem(&Natural::from_u64(2)).0;
    }
    assert_eq!(via.as_limbs(), acc.as_limbs());
}
