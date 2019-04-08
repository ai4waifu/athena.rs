//! Living `13`：`foreign::mpn_oracle` 与 portable 生产路径差分（不进 `KernelTable`）。

use athena_gc::{GcHeap, HeapBudget};
use athena_numeric::{ExecutionBudget, NumericContext, foreign::mpn_oracle, natural::Natural};

fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state
}

fn random_limbs(state: &mut u64, max_len: usize) -> Vec<u64> {
    let len = (lcg_next(state) as usize % max_len) + 1;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let mut limb = lcg_next(state);
        if i + 1 == len {
            while limb == 0 {
                limb = lcg_next(state);
            }
        }
        out.push(limb);
    }
    out
}

#[test]
fn mpn_oracle_add_mul_matches_natural() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let mut seed = 0x4F4D_u64;
    for _ in 0..96 {
        let a_l = random_limbs(&mut seed, 6);
        let b_l = random_limbs(&mut seed, 6);
        let a = Natural::from_limbs_in(&ctx, a_l.clone()).expect("a");
        let b = Natural::from_limbs_in(&ctx, b_l.clone()).expect("b");

        assert_eq!(mpn_oracle::add_n(&a_l, &b_l), a.try_add(&b, &ctx).expect("add").as_limbs());
        assert_eq!(mpn_oracle::mul_n(&a_l, &b_l), a.try_mul(&b, &ctx).expect("mul").as_limbs());
        assert_eq!(mpn_oracle::sqr(&a_l), a.try_mul(&a, &ctx).expect("sqr").as_limbs());
        assert_eq!(mpn_oracle::mul_1(&a_l, b_l[0]), a.try_mul_u64(b_l[0], &ctx).expect("mul1").as_limbs());
    }
}

#[test]
fn mpn_oracle_sub_div_matches_natural() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let mut seed = 0x4D50_u64;
    for _ in 0..64 {
        let mut a_l = random_limbs(&mut seed, 7);
        let b_l = random_limbs(&mut seed, 5);
        if mpn_oracle::cmp_slice(&a_l, &b_l) == std::cmp::Ordering::Less {
            a_l = mpn_oracle::add_n(&a_l, &b_l);
        }
        let a = Natural::from_limbs_in(&ctx, a_l.clone()).expect("a");
        let b = Natural::from_limbs_in(&ctx, b_l.clone()).expect("b");

        assert_eq!(mpn_oracle::sub_n(&a_l, &b_l), a.try_sub(&b, &ctx).expect("sub").as_limbs());

        let (oq, or) = mpn_oracle::div_rem(&a_l, &b_l);
        let (tq, tr) = a.div_rem(&b);
        assert_eq!(oq, tq.as_limbs());
        assert_eq!(or, tr.as_limbs());
        assert_eq!(mpn_oracle::cmp_slice(&or, &b_l), std::cmp::Ordering::Less);
        assert_eq!(mpn_oracle::add_n(&mpn_oracle::mul_n(&oq, &b_l), &or), mpn_oracle::normalize(&a_l));
    }
}

#[test]
fn mpn_oracle_div_rem_single_limb_identity() {
    let u = vec![u64::MAX, u64::MAX, 7];
    let v = vec![3];
    let (q, r) = mpn_oracle::div_rem(&u, &v);
    assert_eq!(mpn_oracle::add_n(&mpn_oracle::mul_n(&q, &v), &r), mpn_oracle::normalize(&u));
    assert!(r[0] < 3);
}
