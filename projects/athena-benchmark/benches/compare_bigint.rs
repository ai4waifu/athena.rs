//! Criterion：bigint 矩阵唯一性能计时入口。
//!
//! Fixture / layer / context / 正确性在 `athena_benchmark::bigint`；本文件只做 Criterion 包装。
//! `athena-bench` **不再**测量 ns/op。
//!
//! Athena kernel：session bump+clear。  
//! Athena **numeric `Add`/`Mul`**：`Ephemeral*` + `NumericBatch`（不 promote）。  
//! Athena numeric 其余 op 与 kernel 同 bump+clear；`e2e` / peer 测真实 owning / Drop。
//!
//! ```sh
//! 运行：`cargo bench -p athena-benchmark --features compare-bigint --bench compare_bigint`
//! ```

#![allow(missing_docs)]

use std::hint::black_box;

use athena_benchmark::bigint::{BigIntOp, cases_for_op, prepare};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn bench_op(c: &mut Criterion, op: BigIntOp) {
    let mut group = c.benchmark_group(format!("bigint_{}", op.as_str()));
    for case in cases_for_op(op) {
        let prepared = prepare(case);
        // 合同校验在热路径外；失败应使 bench 进程直接退出。
        if let Err(e) = prepared.validate() {
            panic!("bigint fixture validation failed for {}: {e}", case.id());
        }
        group.bench_with_input(BenchmarkId::new(case.criterion_function(), case.bits), &prepared, |bencher, prepared| {
            bencher.iter_custom(|iters| {
                let elapsed = prepared.run_timed_batch(iters);
                black_box(());
                elapsed
            });
        });
    }
    group.finish();
}

fn bench_add(c: &mut Criterion) {
    bench_op(c, BigIntOp::Add);
}
fn bench_mul(c: &mut Criterion) {
    bench_op(c, BigIntOp::Mul);
}
fn bench_div(c: &mut Criterion) {
    bench_op(c, BigIntOp::Div);
}
fn bench_gcd(c: &mut Criterion) {
    bench_op(c, BigIntOp::Gcd);
}
fn bench_pow(c: &mut Criterion) {
    bench_op(c, BigIntOp::Pow);
}

criterion_group!(benches, bench_add, bench_mul, bench_div, bench_gcd, bench_pow);
criterion_main!(benches);
