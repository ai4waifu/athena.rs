//! Criterion adapter：调用 `athena_benchmark::bigint` 统一运行器。
//!
//! 输入、操作语义、分层与正确性均在 `src/bigint/`；本文件只做 Criterion 计时包装。
//!
//! ```sh
//! cargo bench -p athena-benchmark --features compare-bigint --bench compare_bigint
//! ```

#![allow(missing_docs)]

use std::hint::black_box;

use athena_benchmark::bigint::{BigIntOp, cases_for_op, prepare};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn bench_op(c: &mut Criterion, op: BigIntOp) {
    let mut group = c.benchmark_group(format!("bigint_{}", op.as_str()));
    for case in cases_for_op(op) {
        let prepared = prepare(case);
        group.bench_with_input(BenchmarkId::new(case.criterion_function(), case.bits), &prepared, |bencher, prepared| {
            bencher.iter(|| {
                prepared.run_once();
                black_box(());
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
