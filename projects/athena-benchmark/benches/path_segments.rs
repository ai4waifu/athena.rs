//! Criterion：Living 18 path 分段（~256-bit）性能计时。
//!
//! 与 `athena-bench --groups path` 共享同一操作语义；后者只做合同 / 资源，不计 ns/op。
//!
//! ```sh
//! cargo bench -p athena-benchmark --bench path_segments
//! ```

#![allow(missing_docs)]

use std::{hint::black_box, str::FromStr};

use athena_gc::{GcHeap, GcMode, HeapBudget};
use athena_numeric::{ExecutionBudget, Integer, NumericContext, natural::Natural};
use criterion::{Criterion, criterion_group, criterion_main};

const LIMBS4: [u64; 4] = [0x1111_1111_1111_1111, 0x2222_2222_2222_2222, 0x3333_3333_3333_3333, 0x4444_4444_4444_4444];
const LIMBS4_B: [u64; 4] =
    [0x5555_5555_5555_5555, 0x6666_6666_6666_6666, 0x7777_7777_7777_7777, 0x0000_0000_0000_0001];

fn stack_add4(a: &[u64; 4], b: &[u64; 4]) -> [u64; 5] {
    let mut out = [0u64; 5];
    let mut carry = 0u64;
    for i in 0..4 {
        let (s, c1) = a[i].overflowing_add(b[i]);
        let (s, c2) = s.overflowing_add(carry);
        out[i] = s;
        carry = u64::from(c1) + u64::from(c2);
    }
    out[4] = carry;
    out
}

fn make_ctx(mode: GcMode) -> NumericContext {
    let heap = GcHeap::new_shared(HeapBudget::default());
    heap.borrow().gc().set_base_mode(mode);
    NumericContext::with_heap(ExecutionBudget::unlimited(), heap)
}

fn bench_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_segments_4limb");

    group.bench_function("stack_add_4", |b| {
        b.iter(|| black_box(stack_add4(black_box(&LIMBS4), black_box(&LIMBS4_B))));
    });

    {
        let ctx = make_ctx(GcMode::Disabled);
        let a = Natural::from_limbs_in(&ctx, LIMBS4.to_vec()).expect("a");
        let b = Natural::from_limbs_in(&ctx, LIMBS4_B.to_vec()).expect("b");
        group.bench_function("natural_try_add_disabled", |bencher| {
            bencher.iter(|| black_box(a.try_add(black_box(&b), black_box(&ctx)).expect("add")));
        });
    }

    {
        let ctx = make_ctx(GcMode::Disabled);
        let a_dec = Natural::from_limbs_in(&ctx, LIMBS4.to_vec()).unwrap().to_decimal_string();
        let b_dec = Natural::from_limbs_in(&ctx, LIMBS4_B.to_vec()).unwrap().to_decimal_string();
        // Integer 操作数挂在 session Disabled ctx 上构造，避免 e2e shared Auto 混入。
        let a = Integer::from_str(&a_dec).expect("a");
        let b = Integer::from_str(&b_dec).expect("b");
        let session = make_ctx(GcMode::Disabled);
        let a_s = a.try_add(&Integer::zero(), &session).expect("repub a");
        let b_s = b.try_add(&Integer::zero(), &session).expect("repub b");
        group.bench_function("integer_try_add_session_disabled", |bencher| {
            bencher.iter(|| black_box(a_s.try_add(black_box(&b_s), black_box(&session)).expect("add")));
        });
    }

    {
        let a_dec = Natural::from_limbs(LIMBS4.to_vec()).unwrap().to_decimal_string();
        let b_dec = Natural::from_limbs(LIMBS4_B.to_vec()).unwrap().to_decimal_string();
        let a = Integer::from_str(&a_dec).expect("a");
        let b = Integer::from_str(&b_dec).expect("b");
        let ctx = NumericContext::unlimited();
        group.bench_function("integer_try_add_shared_auto", |bencher| {
            bencher.iter(|| black_box(a.try_add(black_box(&b), black_box(&ctx)).expect("add")));
        });
        group.bench_function("integer_add_e2e", |bencher| {
            bencher.iter(|| black_box(a.add(black_box(&b))));
        });
        group.bench_function("integer_clone_shared_auto", |bencher| {
            bencher.iter(|| black_box(a.clone()));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_path);
criterion_main!(benches);
