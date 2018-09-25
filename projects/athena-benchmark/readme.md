# `athena-benchmark`

测量 Athena 内核在固定 fixture 上的性能与资源开销。基准结果不定义数学语义或稳定 API；正确性校验优先于计时。

**不进主 CI。** 仅本地或单独手动流程运行。Mathematica / MATLAB 等外部对照属于 SXO 本地 opt-in（`bench:local`），不是本 crate
的职责。

## 两套入口（分工）

| 入口 | 工具 | 用途 |
|------|------|------|
| `athena-bench` | 自有 fixture + JSON 报告 | 带结果 / diagnostic / exactness 校验的内核合同基准（Living `17`） |
| `cargo bench` | **Criterion** | 微基准与外部库 PK（吞吐、相对比、HTML 报告） |

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

## Criterion：Athena vs `num-bigint` vs `ibig`

`ramp` 已弃用且依赖 nightly asm，**不**纳入可复现对照；纯 Rust 高性能对照用 `ibig`。

```sh
cargo bench -p athena-benchmark --features bigint-compare --bench bigint_compare
```

覆盖 `add` / `mul` / `div` / `gcd` / `pow`，位宽 `64 / 256 / 1024 / 4096`。报告在 `target/criterion/`（含 HTML）。

外部 bigint 依赖仅挂在本 crate 的可选 feature 上，**不得**回流到 `athena-types` / `athena-numeric` / `athena-engine`。
