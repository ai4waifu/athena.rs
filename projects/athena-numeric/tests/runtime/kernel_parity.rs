//! `KernelTable` 绑定与 pure / ISA parity。

use athena_gc::{GcHeap, HeapBudget};
use athena_numeric::{
    CapabilityBundle, ExecutionBudget, ExecutionToken, KernelTable, MachineCapability, NumericContext, natural::Natural,
};

#[test]
fn pure_rust_context_binds_pure_kernel_table() {
    let ctx = NumericContext::pure_rust_default();
    assert_eq!(ctx.kernels().id(), "pure_rust");
    assert_eq!(ctx.capabilities().machine, MachineCapability::PURE_RUST);
}

#[test]
fn capability_bundle_selects_kernel_at_context_creation() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let mut caps = CapabilityBundle::pure_rust_default();
    caps.machine.adx = true;
    let ctx = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap, caps);
    #[cfg(all(target_arch = "x86_64", not(target_family = "wasm")))]
    assert_eq!(ctx.kernels().id(), "x86_64_adx");
    #[cfg(not(all(target_arch = "x86_64", not(target_family = "wasm"))))]
    assert_eq!(ctx.kernels().id(), "pure_rust");
}

#[test]
fn pure_and_bound_tables_agree_on_add_mul() {
    let pure = KernelTable::pure_rust();
    let bound = KernelTable::bind(MachineCapability { adx: true, bmi2: true, ..MachineCapability::PURE_RUST });

    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx_pure = NumericContext::with_heap(ExecutionBudget::unlimited(), heap.clone()).with_pure_rust_kernels();
    let ctx_isa = NumericContext::with_capabilities(
        ExecutionBudget::unlimited(),
        heap,
        CapabilityBundle {
            machine: MachineCapability { adx: true, bmi2: true, ..MachineCapability::PURE_RUST },
            ..CapabilityBundle::pure_rust_default()
        },
    );

    let samples: &[&[u64]] = &[&[1], &[u64::MAX], &[1, 2], &[u64::MAX, u64::MAX], &[1, 2, 3, 4], &[u64::MAX, 1, 2, 3]];

    for a in samples {
        for b in samples {
            let na = Natural::from_limbs_in(&ctx_pure, a.to_vec()).expect("a");
            let nb = Natural::from_limbs_in(&ctx_pure, b.to_vec()).expect("b");
            let sum_p = na.try_add(&nb, &ctx_pure).expect("add pure");
            let sum_i = na.try_add(&nb, &ctx_isa).expect("add isa");
            assert_eq!(sum_p.as_limbs(), sum_i.as_limbs(), "add parity {a:?} + {b:?}");

            let prod_p = na.try_mul(&nb, &ctx_pure).expect("mul pure");
            let prod_i = na.try_mul(&nb, &ctx_isa).expect("mul isa");
            assert_eq!(prod_p.as_limbs(), prod_i.as_limbs(), "mul parity {a:?} * {b:?}");

            if na >= nb {
                let sub_p = na.try_sub(&nb, &ctx_pure).expect("sub pure");
                let sub_i = na.try_sub(&nb, &ctx_isa).expect("sub isa");
                assert_eq!(sub_p.as_limbs(), sub_i.as_limbs(), "sub parity {a:?} - {b:?}");
            }

            let mul1_p = na.try_mul_u64(7, &ctx_pure).expect("mul1 pure");
            let mul1_i = na.try_mul_u64(7, &ctx_isa).expect("mul1 isa");
            assert_eq!(mul1_p.as_limbs(), mul1_i.as_limbs(), "mul_1 parity {a:?} * 7");
        }
    }

    let tok = ExecutionToken::unverified_for_tests();
    for &(a, b) in &[(1u64, 2), (u64::MAX, 1), (u64::MAX, u64::MAX)] {
        assert_eq!(pure.add_1(tok, a, b), bound.add_1(tok, a, b));
        assert_eq!(pure.mul_1x1(tok, a, b), bound.mul_1x1(tok, a, b));
    }
}

#[test]
fn algorithm_planner_picks_karatsuba_and_toom_by_width() {
    use athena_numeric::algorithm::{
        AlgorithmPlanner, DIV_BZ_THRESHOLD, DivStrategy, MUL_KARATSUBA_THRESHOLD, MUL_TOOM_THRESHOLD, MulStrategy,
    };

    let planner = AlgorithmPlanner::new(CapabilityBundle::pure_rust_default());
    assert_eq!(planner.plan_mul(1, 1), MulStrategy::Schoolbook);
    assert_eq!(planner.plan_mul(MUL_KARATSUBA_THRESHOLD, MUL_KARATSUBA_THRESHOLD), MulStrategy::Karatsuba);
    assert_eq!(planner.plan_mul(MUL_TOOM_THRESHOLD, MUL_TOOM_THRESHOLD), MulStrategy::Toom3);
    assert_eq!(planner.plan_div(8, 4), DivStrategy::Knuth);
    assert_eq!(planner.plan_div(DIV_BZ_THRESHOLD.max(128), 32), DivStrategy::BurnikelZiegler);
}

#[test]
fn toom_width_mul_matches_schoolbook() {
    let ctx = NumericContext::unlimited();
    // 略高于 Toom 阈值：强制走三路分块路径。
    let limbs_a: Vec<u64> = (1u64..=100).collect();
    let limbs_b: Vec<u64> = (3u64..=102).collect();
    let a = Natural::from_limbs_in(&ctx, limbs_a).expect("a");
    let b = Natural::from_limbs_in(&ctx, limbs_b).expect("b");
    let prod = a.try_mul(&b, &ctx).expect("mul");

    // 对照：强制仅 schoolbook（关闭 karatsuba/toom）。
    let mut caps = CapabilityBundle::pure_rust_default();
    caps.algorithm.karatsuba = false;
    caps.algorithm.toom = false;
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx_sb = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap, caps);
    let prod_sb = a.try_mul(&b, &ctx_sb).expect("schoolbook");
    assert_eq!(prod.as_limbs(), prod_sb.as_limbs());
}
