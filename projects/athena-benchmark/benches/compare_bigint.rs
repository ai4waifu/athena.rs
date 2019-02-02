//! Athena `Integer` vs `num-bigint` vs `ibig` vs `malachite`（Criterion）。
//!
//! 仅纯 Rust 对照。GMP / `gmp-mpfr-sys` 在 Windows 上依赖 MSYS2 + `*-windows-gnu`，本仓库不接。
//!
//! ```sh
//! 运行：`cargo bench -p athena-benchmark --features compare-bigint --bench compare_bigint`
//! ```

#![allow(missing_docs)]

use std::{hint::black_box, str::FromStr};

use athena_numeric::{Integer, number_from_wire};
use athena_types::wire::WireNumber;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ibig::{IBig, ops::Abs as IbigAbs};
use malachite::{
    Integer as MalachiteInteger,
    base::num::arithmetic::traits::{Abs as MalachiteAbs, Pow as MalachitePow},
};
use num_bigint::BigInt;
use num_traits::{Num, Signed, Zero, pow::Pow};

const BITS: &[u32] = &[64, 256, 1024, 4096];

fn gen_decimal(bits: u32, seed: u64) -> String {
    let mut n = Integer::from_u64(seed | 1);
    let mix = Integer::from_u64(0x9E37_79B9_7F4A_7C15);
    while n.bits() < u64::from(bits) {
        n = n.mul(&mix).add(&Integer::from_u64(0xD1B5_4A32_D192_ED03));
    }
    while n.bits() > u64::from(bits) {
        n = n.div(&Integer::from_i64(2)).expect("shrink bits");
    }
    if n.is_zero() {
        n = Integer::one();
    }
    n.to_decimal_string()
}

fn operands(bits: u32) -> (String, String) {
    let a = gen_decimal(bits, 0xC0FF_EE00_D15E_A5EDu64.wrapping_add(u64::from(bits)));
    let b = gen_decimal(bits, 0xDEAD_BEEF_F00D_CAFEu64.wrapping_add(u64::from(bits) * 17));
    (a, b)
}

fn pow_exp(bits: u32) -> u32 {
    match bits {
        64 => 17,
        256 => 9,
        1024 => 5,
        _ => 3,
    }
}

fn gcd_num(a: &BigInt, b: &BigInt) -> BigInt {
    let mut x = a.abs();
    let mut y = b.abs();
    while !y.is_zero() {
        let r = &x % &y;
        x = y;
        y = r;
    }
    x
}

fn gcd_ibig(a: &IBig, b: &IBig) -> IBig {
    let zero = IBig::from(0);
    let mut x = IbigAbs::abs(a.clone());
    let mut y = IbigAbs::abs(b.clone());
    while y != zero {
        let r = &x % &y;
        x = y;
        y = r;
    }
    x
}

fn gcd_malachite(a: &MalachiteInteger, b: &MalachiteInteger) -> MalachiteInteger {
    let zero = MalachiteInteger::from(0);
    let mut x = MalachiteAbs::abs(a.clone());
    let mut y = MalachiteAbs::abs(b.clone());
    while y != zero {
        let r = &x % &y;
        x = y;
        y = r;
    }
    x
}

fn integer_from_wire_decimal(s: &str) -> Integer {
    number_from_wire(&WireNumber::from_decimal_str(s).unwrap()).unwrap().as_integer().unwrap().clone()
}

fn malachite_from_decimal(s: &str) -> MalachiteInteger {
    MalachiteInteger::from_str(s).expect("malachite parse")
}

fn bench_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("bigint_add");
    for &bits in BITS {
        let (a_s, b_s) = operands(bits);
        let a_ath = integer_from_wire_decimal(&a_s);
        let b_ath = integer_from_wire_decimal(&b_s);
        let a_num = BigInt::from_str_radix(&a_s, 10).unwrap();
        let b_num = BigInt::from_str_radix(&b_s, 10).unwrap();
        let a_ibig: IBig = a_s.parse().unwrap();
        let b_ibig: IBig = b_s.parse().unwrap();
        let a_mal = malachite_from_decimal(&a_s);
        let b_mal = malachite_from_decimal(&b_s);

        group.bench_with_input(BenchmarkId::new("athena", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(a_ath.add(&b_ath)));
        });
        group.bench_with_input(BenchmarkId::new("num", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(&a_num + &b_num));
        });
        group.bench_with_input(BenchmarkId::new("ibig", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(&a_ibig + &b_ibig));
        });
        group.bench_with_input(BenchmarkId::new("malachite", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(&a_mal + &b_mal));
        });
    }
    group.finish();
}

fn bench_mul(c: &mut Criterion) {
    let mut group = c.benchmark_group("bigint_mul");
    for &bits in BITS {
        let (a_s, b_s) = operands(bits);
        let a_ath = integer_from_wire_decimal(&a_s);
        let b_ath = integer_from_wire_decimal(&b_s);
        let a_num = BigInt::from_str_radix(&a_s, 10).unwrap();
        let b_num = BigInt::from_str_radix(&b_s, 10).unwrap();
        let a_ibig: IBig = a_s.parse().unwrap();
        let b_ibig: IBig = b_s.parse().unwrap();
        let a_mal = malachite_from_decimal(&a_s);
        let b_mal = malachite_from_decimal(&b_s);

        group.bench_with_input(BenchmarkId::new("athena", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(a_ath.mul(&b_ath)));
        });
        group.bench_with_input(BenchmarkId::new("num", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(&a_num * &b_num));
        });
        group.bench_with_input(BenchmarkId::new("ibig", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(&a_ibig * &b_ibig));
        });
        group.bench_with_input(BenchmarkId::new("malachite", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(&a_mal * &b_mal));
        });
    }
    group.finish();
}

fn bench_div(c: &mut Criterion) {
    let mut group = c.benchmark_group("bigint_div");
    for &bits in BITS {
        let (a_s, b_s) = operands(bits);
        let a_ath = integer_from_wire_decimal(&a_s);
        let b_ath = integer_from_wire_decimal(&b_s);
        let prod_ath = a_ath.mul(&b_ath);
        let a_num = BigInt::from_str_radix(&a_s, 10).unwrap();
        let b_num = BigInt::from_str_radix(&b_s, 10).unwrap();
        let prod_num = &a_num * &b_num;
        let a_ibig: IBig = a_s.parse().unwrap();
        let b_ibig: IBig = b_s.parse().unwrap();
        let prod_ibig = &a_ibig * &b_ibig;
        let a_mal = malachite_from_decimal(&a_s);
        let b_mal = malachite_from_decimal(&b_s);
        let prod_mal = &a_mal * &b_mal;

        group.bench_with_input(BenchmarkId::new("athena", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(prod_ath.div(&a_ath)));
        });
        group.bench_with_input(BenchmarkId::new("num", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(&prod_num / &a_num));
        });
        group.bench_with_input(BenchmarkId::new("ibig", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(&prod_ibig / &a_ibig));
        });
        group.bench_with_input(BenchmarkId::new("malachite", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(&prod_mal / &a_mal));
        });
        let _ = (&b_ath, &b_num, &b_ibig, &b_mal);
    }
    group.finish();
}

fn bench_gcd(c: &mut Criterion) {
    let mut group = c.benchmark_group("bigint_gcd");
    for &bits in BITS {
        let (a_s, b_s) = operands(bits);
        let a_ath = integer_from_wire_decimal(&a_s);
        let b_ath = integer_from_wire_decimal(&b_s);
        let a_num = BigInt::from_str_radix(&a_s, 10).unwrap();
        let b_num = BigInt::from_str_radix(&b_s, 10).unwrap();
        let a_ibig: IBig = a_s.parse().unwrap();
        let b_ibig: IBig = b_s.parse().unwrap();
        let a_mal = malachite_from_decimal(&a_s);
        let b_mal = malachite_from_decimal(&b_s);

        group.bench_with_input(BenchmarkId::new("athena", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(a_ath.gcd(&b_ath)));
        });
        group.bench_with_input(BenchmarkId::new("num", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(gcd_num(&a_num, &b_num)));
        });
        group.bench_with_input(BenchmarkId::new("ibig", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(gcd_ibig(&a_ibig, &b_ibig)));
        });
        group.bench_with_input(BenchmarkId::new("malachite", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(gcd_malachite(&a_mal, &b_mal)));
        });
    }
    group.finish();
}

fn bench_pow(c: &mut Criterion) {
    let mut group = c.benchmark_group("bigint_pow");
    for &bits in BITS {
        let (a_s, _) = operands(bits);
        let exp = pow_exp(bits);
        let a_ath = integer_from_wire_decimal(&a_s);
        let e_ath = Integer::from_u64(u64::from(exp));
        let a_num = BigInt::from_str_radix(&a_s, 10).unwrap();
        let a_ibig: IBig = a_s.parse().unwrap();
        let a_mal = malachite_from_decimal(&a_s);
        let exp_u64 = u64::from(exp);

        group.bench_with_input(BenchmarkId::new("athena", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(a_ath.pow(&e_ath).expect("pow")));
        });
        group.bench_with_input(BenchmarkId::new("num", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(Pow::pow(&a_num, exp)));
        });
        group.bench_with_input(BenchmarkId::new("ibig", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(a_ibig.pow(usize::try_from(exp).unwrap())));
        });
        group.bench_with_input(BenchmarkId::new("malachite", bits), &bits, |bencher, _| {
            bencher.iter(|| black_box(MalachitePow::pow(a_mal.clone(), exp_u64)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_add, bench_mul, bench_div, bench_gcd, bench_pow);
criterion_main!(benches);
