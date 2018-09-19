# `athena-benchmark`

测量 Athena 内核在固定 fixture 上的性能与资源开销。基准结果不定义数学语义或稳定 API；正确性校验优先于计时。

**不进主 CI。** 仅本地或单独手动流程运行。Mathematica / MATLAB 等外部对照属于 SXO 本地 opt-in（`bench:local`），不是本 crate
的职责。

## 分组

| 组         | 内容                                                      |
|------------|-----------------------------------------------------------|
| `numeric`  | ExactInteger / ExactRational / promotion                  |
| `ir`       | `TermArena`、`canonical_hash`、`verify`                   |
| `rewriter` | simplify / 规范化种子                                     |
| `engine`   | eval 热路径                                               |
| `domains`  | `sample_1d` 等域算法                                      |
| `jit`      | 仅 `--features jit`；当前无 `athena-jit` 时标记 `skipped` |

## 本地运行

```sh
cargo run -p athena-benchmark --release -- --groups numeric,ir --json
cargo run -p athena-benchmark --release -- --warmup 5 --samples 50
```

报告字段包括：commit、rustc、target、CPU、线程数、JIT 开关、warmup/samples、p50/p95、校验摘要、回退原因。分配 / 峰值内存尽力填写，拿不到则为
`null`。
