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

Athena 热路径在迭代外复用同一个 `NumericContext`，调用 `try_add` / `try_mul` / …，
**不**把每次 `Integer::add` 里重建 `pure_rust_default()` 算进算法时间。仍走完整
`Integer` 幅度运算与 heap 发布，不按位宽切换 kernel 快路径。

外部 bigint 依赖仅挂在本 crate 的可选 `compare-*` feature 上， **不得**回流到 `athena-types` / `athena-numeric` /
`athena-engine`。

## 最近一次 `compare-bigint` 成绩

> **作废待重跑**：下列数字含每次运算重建 `NumericContext` 的开销，不能代表复用
> context 后的算法对比。重跑后再填。

- 机器：Windows · `cargo bench --release` · Criterion median
- 参数：`--sample-size 40 --warm-up-time 0.3 --measurement-time 1.0`
- 日期：2026-09-02
- 相对比以 **athena = 1×**（`peer / athena`，越小越快）。`bigint_pow` / 4096 指数为 `2`。
- Criterion 的 `change%` 是相对本机旧 base，改指数或环境后不要当加速证据。

### `bigint_add`

| bits |   athena | num-bigint |     ibig | malachite |
|-----:|---------:|-----------:|---------:|----------:|
|   64 | 164.1 ns |    64.1 ns |  70.9 ns |   68.4 ns |
|  256 |  5.08 µs |   133.2 ns |  78.5 ns |  162.5 ns |
| 1024 |  5.41 µs |   154.6 ns |  85.4 ns |  167.3 ns |
| 4096 |  5.21 µs |   198.0 ns | 138.0 ns |  217.8 ns |

### `bigint_mul`

| bits |   athena | num-bigint |     ibig | malachite |
|-----:|---------:|-----------:|---------:|----------:|
|   64 | 151.3 ns |    70.4 ns |  73.7 ns |   65.5 ns |
|  256 |  5.85 µs |   103.3 ns | 123.7 ns |  120.3 ns |
| 1024 |  6.07 µs |   489.0 ns | 483.6 ns |  533.6 ns |
| 4096 | 12.90 µs |    5.96 µs |  5.23 µs |   3.80 µs |

### `bigint_div`

| bits |   athena | num-bigint |     ibig | malachite |
|-----:|---------:|-----------:|---------:|----------:|
|   64 | 200.0 ns |    74.0 ns |  92.7 ns |   81.2 ns |
|  256 |  6.20 µs |   241.0 ns | 290.2 ns |  186.9 ns |
| 1024 |  6.93 µs |   872.9 ns | 917.0 ns |  645.3 ns |
| 4096 | 17.86 µs |    9.58 µs |  9.38 µs |   6.86 µs |

### `bigint_gcd`

| bits |    athena | num-bigint |     ibig | malachite |
|-----:|----------:|-----------:|---------:|----------:|
|   64 |   5.82 µs |   714.2 ns | 159.5 ns |  516.1 ns |
|  256 |  30.70 µs |   31.76 µs | 16.57 µs |  28.37 µs |
| 1024 | 124.07 µs |   42.82 µs | 27.31 µs |  49.24 µs |
| 4096 | 702.12 µs |   55.74 µs | 51.42 µs |  38.67 µs |

### `bigint_pow`

| bits |   athena | num-bigint |     ibig | malachite |
|-----:|---------:|-----------:|---------:|----------:|
|   64 |  9.46 µs |   645.4 ns | 559.5 ns |  415.6 ns |
|  256 | 13.16 µs |    1.17 µs |  1.06 µs |  993.5 ns |
| 1024 | 22.78 µs |    4.01 µs |  4.01 µs |   3.74 µs |
| 4096 | 42.83 µs |    5.48 µs |  4.94 µs |   3.65 µs |

### 相对 athena（athena = 1×，值 = peer / athena，越小越快）

| op  | lib    |    64 |   256 |  1024 |  4096 |
|-----|--------|------:|------:|------:|------:|
| add | athena |    1× |    1× |    1× |    1× |
| add | num    | 0.39× | 0.03× | 0.03× | 0.04× |
| add | ibig   | 0.43× | 0.02× | 0.02× | 0.03× |
| add | mal    | 0.42× | 0.03× | 0.03× | 0.04× |
| mul | athena |    1× |    1× |    1× |    1× |
| mul | num    | 0.47× | 0.02× | 0.08× | 0.46× |
| mul | ibig   | 0.49× | 0.02× | 0.08× | 0.41× |
| mul | mal    | 0.43× | 0.02× | 0.09× | 0.29× |
| div | athena |    1× |    1× |    1× |    1× |
| div | num    | 0.37× | 0.04× | 0.13× | 0.54× |
| div | ibig   | 0.46× | 0.05× | 0.13× | 0.53× |
| div | mal    | 0.41× | 0.03× | 0.09× | 0.38× |
| gcd | athena |    1× |    1× |    1× |    1× |
| gcd | num    | 0.12× |  1.0× | 0.35× | 0.08× |
| gcd | ibig   | 0.03× | 0.54× | 0.22× | 0.07× |
| gcd | mal    | 0.09× | 0.92× | 0.40× | 0.06× |
| pow | athena |    1× |    1× |    1× |    1× |
| pow | num    | 0.07× | 0.09× | 0.18× | 0.13× |
| pow | ibig   | 0.06× | 0.08× | 0.18× | 0.12× |
| pow | mal    | 0.04× | 0.08× | 0.16× | 0.09× |
