//! Criterion：分配路径对照（raw bump / header / batch / Drop）。
//!
//! 与 bigint 算术矩阵正交：本文件回答「慢在 bump 还是 header/stats/RefCell」。
//!
//! ```sh
//! cargo bench -p athena-benchmark --bench allocation_modes
//! ```

#![allow(missing_docs)]

use std::{hint::black_box, time::Instant};

use athena_gc::{AllocationAccounting, GcHeap, GcMode, HeapBudget};
use criterion::{Criterion, criterion_group, criterion_main};

const LIMBS4: [u64; 4] = [0x1111_1111_1111_1111, 0x2222_2222_2222_2222, 0x3333_3333_3333_3333, 0x4444_4444_4444_4444];
const LIMBS4_B: [u64; 4] = [0x5555_5555_5555_5555, 0x6666_6666_6666_6666, 0x7777_7777_7777_7777, 0x0000_0000_0000_0001];

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

fn make_heap() -> std::rc::Rc<std::cell::RefCell<GcHeap>> {
    let heap = GcHeap::new_shared(HeapBudget::for_microbench());
    heap.borrow().gc().set_base_mode(GcMode::Disabled);
    heap
}

fn bench_allocation_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_modes");

    group.bench_function("A_kernel_stack_add_4", |b| {
        b.iter(|| black_box(stack_add4(black_box(&LIMBS4), black_box(&LIMBS4_B))));
    });

    {
        let heap = make_heap();
        group.bench_function("B_raw_bump_32B", |bencher| {
            bencher.iter_custom(|iters| {
                let mut h = heap.borrow_mut();
                h.enable_bump_ephemeral(true);
                h.with_accounting(AllocationAccounting::Off, |h| {
                    let mark = h.mark_numeric_bump();
                    let start = Instant::now();
                    for _ in 0..iters {
                        let p = h.bench_bump_raw_bytes(32).expect("bump");
                        black_box(p);
                    }
                    let elapsed = start.elapsed();
                    h.clear_numeric_to(mark).expect("clear");
                    elapsed
                })
            });
        });
    }

    {
        let heap = make_heap();
        group.bench_function("B_header_alloc_off_4limbs", |bencher| {
            bencher.iter_custom(|iters| {
                let mut h = heap.borrow_mut();
                h.enable_bump_ephemeral(true);
                h.with_accounting(AllocationAccounting::Off, |h| {
                    let mark = h.mark_numeric_bump();
                    let start = Instant::now();
                    for _ in 0..iters {
                        let block = h.allocate_numeric_block(4).expect("alloc");
                        black_box(block.ptr);
                        let _ = block;
                    }
                    let elapsed = start.elapsed();
                    h.clear_numeric_to(mark).expect("clear");
                    elapsed
                })
            });
        });
    }

    {
        let heap = make_heap();
        group.bench_function("C_batch_alloc_4limbs", |bencher| {
            bencher.iter_custom(|iters| {
                let mut h = heap.borrow_mut();
                let start = Instant::now();
                h.with_numeric_batch(|batch| {
                    for _ in 0..iters {
                        let block = batch.allocate_limbs(4).expect("alloc");
                        black_box(block.ptr);
                        let _ = block;
                    }
                })
                .expect("batch");
                start.elapsed()
            });
        });
    }

    {
        let heap = make_heap();
        group.bench_function("C_numeric_batch_add_256b", |bencher| {
            bencher.iter_custom(|iters| {
                let mut h = heap.borrow_mut();
                let start = Instant::now();
                h.with_numeric_batch(|batch| {
                    for _ in 0..iters {
                        let n = athena_numeric::TemporaryNatural::try_add(&LIMBS4, &LIMBS4_B, batch).expect("add");
                        black_box(n.as_limbs());
                        drop(n);
                    }
                })
                .expect("batch");
                start.elapsed()
            });
        });
    }

    {
        // promote 必须写到与 batch 不同的 heap（同 heap 会被 rewind 抹掉）。
        let batch_heap = make_heap();
        let persist = athena_numeric::NumericContext::session_with_heap_budget(HeapBudget::for_microbench());
        persist.heap().borrow().gc().set_base_mode(GcMode::Disabled);
        group.bench_function("D_promote_ephemeral_natural_4limbs", |bencher| {
            bencher.iter_custom(|iters| {
                let mut total = std::time::Duration::ZERO;
                for _ in 0..iters {
                    let mut h = batch_heap.borrow_mut();
                    let promoted = h
                        .with_numeric_batch(|batch| {
                            let e = athena_numeric::TemporaryNatural::try_add(&LIMBS4, &LIMBS4_B, batch).expect("add");
                            let start = Instant::now();
                            let n = e.promote(&persist).expect("promote");
                            let elapsed = start.elapsed();
                            black_box(n.as_limbs());
                            elapsed
                        })
                        .expect("batch");
                    total += promoted;
                }
                total
            });
        });
    }

    {
        let heap = make_heap();
        group.bench_function("E_full_alloc_drop_4limbs", |bencher| {
            bencher.iter(|| {
                let mut h = heap.borrow_mut();
                let block = h.allocate_numeric_block(4).expect("alloc");
                black_box(block.ptr);
                h.release_numeric_block(block).expect("rel");
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_allocation_modes);
criterion_main!(benches);
