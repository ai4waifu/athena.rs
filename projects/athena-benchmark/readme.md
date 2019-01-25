# `athena-benchmark`

测量 Athena 内核在固定 fixture 上的性能与资源开销。基准结果不定义数学语义或稳定 API；正确性校验优先于计时。

**不进主 CI。** 仅本地或单独手动流程运行。Mathematica / MATLAB 等外部对照属于 SXO 本地 opt-in（`bench:local`），不是本 crate
的职责。

## 两套入口（分工）

| 入口           | 工具                     | 用途                                               |
|----------------|--------------------------|----------------------------------------------------|
| `athena-bench` | 自有 fixture + JSON 报告 | 带结果 / diagnostic / exactness 校验的内核合同基准 |
| `cargo bench`  | **Criterion**            | 微基准与外部库 PK（吞吐、相对比、HTML 报告）       |

合同校验不能用纯 Criterion 替代：CAS 基准必须先验证正确性再计时。外部 bigint 对照则应使用生态标准工具，而不是再造一套计时框架。

## 分组（`athena-bench`）

| 组         | 内容                                                      |
|------------|-----------------------------------------------------------|
| `numeric`  | ExactInteger / ExactRational / promotion                  |
| `ir`       | `TermArena`、`canonical_hash`、`verify`                   |
| `rewriter` | simplify / 规范化种子                                     |
| `engine`   | eval 热路径                                               |
| `domains`  | `sample_1d` 等域算法                                      |
| `jit`      | 仅 `--features jit`；当前无 `athena-jit` 时标记 `skipped` |

```sh
cargo run -p athena-benchmark --release -- --groups numeric,ir --json
cargo run -p athena-benchmark --release -- --warmup 5 --samples 50
```

## Criterion：`compare-bigint`

| Feature              | 对照库                       |
|----------------------|------------------------------|
| `compare-num-bigint` | `num-bigint`                 |
| `compare-ibig`       | `ibig`                       |
| `compare-malachite`  | `malachite`                  |
| `compare-bigint`     | 以上全部（跑完整 PK 用这个） |

不接 GMP / `gmp-mpfr-sys`：Windows 需要 MSYS2 + `windows-gnu`，限制太多。纯 Rust 顶尖对照用 `malachite`。

```sh
cargo bench -p athena-benchmark --features compare-bigint --bench compare_bigint
```

覆盖 `add` / `mul` / `div` / `gcd` / `pow`，位宽 `64 / 256 / 1024 / 4096`。报告在 `target/criterion/`（含 HTML）。

外部 bigint 依赖仅挂在本 crate 的可选 `compare-*` feature 上， **不得**回流到 `athena-types` / `athena-numeric` /
`athena-engine`。

## 最近一次 `compare-bigint` 成绩

- 机器：Windows · `cargo bench --release` · Criterion median
- 参数：`--sample-size 40 --warm-up-time 0.3 --measurement-time 1.0`
- 日期：2026-09-02
- `bigint_pow` / 4096：曾因 Karatsuba `next_power_of_two` 切分导致 scratch `mid > len` panic；已改为 `m = ceil(n/2)` 并整片清零临时区。请重跑 Criterion 更新本表。

### `bigint_add`

| bits |   athena | num-bigint |     ibig | malachite |
|-----:|---------:|-----------:|---------:|----------:|
|   64 | 146.7 ns |    61.8 ns |  73.3 ns |   66.0 ns |
|  256 | 146.6 ns |   152.2 ns |  89.7 ns |  166.5 ns |
| 1024 | 187.3 ns |   204.5 ns |  98.0 ns |  212.2 ns |
| 4096 | 337.8 ns |   297.9 ns | 148.0 ns |  252.1 ns |

### `bigint_mul`

| bits |   athena | num-bigint |     ibig | malachite |
|-----:|---------:|-----------:|---------:|----------:|
|   64 | 275.6 ns |    80.8 ns |  79.7 ns |   70.2 ns |
|  256 | 318.3 ns |   123.2 ns | 127.7 ns |  146.2 ns |
| 1024 | 924.8 ns |   550.1 ns | 565.1 ns |  588.1 ns |
| 4096 |  8.09 µs |    6.37 µs |  5.35 µs |   4.27 µs |

### `bigint_div`

| bits |   athena | num-bigint |     ibig | malachite |
|-----:|---------:|-----------:|---------:|----------:|
|   64 | 385.2 ns |    92.3 ns | 101.4 ns |   82.0 ns |
|  256 | 517.7 ns |   292.0 ns | 357.8 ns |  221.2 ns |
| 1024 |  1.50 µs |   969.4 ns | 994.3 ns |  667.7 ns |
| 4096 | 12.73 µs |   11.31 µs | 10.36 µs |   7.98 µs |

### `bigint_gcd`

| bits |    athena | num-bigint |     ibig | malachite |
|-----:|----------:|-----------:|---------:|----------:|
|   64 |   8.98 µs |   837.8 ns | 179.3 ns |  514.8 ns |
|  256 |  36.04 µs |   40.84 µs | 19.48 µs |  35.17 µs |
| 1024 | 170.27 µs |   56.30 µs | 41.78 µs |  48.40 µs |
| 4096 |   1.08 ms |   69.06 µs | 67.75 µs |  49.50 µs |

### `bigint_pow`

| bits |   athena | num-bigint |     ibig | malachite |
|-----:|---------:|-----------:|---------:|----------:|
|   64 |  6.83 µs |   758.1 ns | 886.2 ns |  457.0 ns |
|  256 |  7.57 µs |    1.42 µs |  1.42 µs |   1.06 µs |
| 1024 | 23.85 µs |    4.41 µs |  4.55 µs |   4.23 µs |
| 4096 |        × |          × |        × |         × |

### 相对 malachite（athena / malachite）

| op \ bits |   64 |   256 |  1024 | 4096 |
|-----------|-----:|------:|------:|-----:|
| add       | 2.2× | 0.88× | 0.88× | 1.3× |
| mul       | 3.9× |  2.2× |  1.6× | 1.9× |
| div       | 4.7× |  2.3× |  2.2× | 1.6× |
| gcd       |  17× |  1.0× |  3.5× |  22× |
| pow       |  15× |  7.1× |  5.6× |    × |
