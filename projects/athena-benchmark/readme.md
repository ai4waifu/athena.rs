# `athena-benchmark`

测量 Athena 内核在固定 fixture 上的性能与资源开销。基准结果不定义数学语义或稳定 API；正确性校验优先于计时。

**不进主 CI。** 仅本地或单独手动流程运行。Mathematica / MATLAB 等外部对照属于 SXO 本地 opt-in（`bench:local`），不是本 crate 的职责。

## 两套入口（分工）

| 入口 | 工具 | 用途 |
|------|------|------|
| `athena-bench` | 自有 fixture + JSON/Markdown | 带校验的内核合同基准（含 path / bigint 分层） |
| `cargo bench` | **Criterion** | 微基准与外部库 PK（吞吐、相对比、HTML） |

合同校验不能用纯 Criterion 替代。外部 bigint 对照用生态标准工具，不在核心 crate 再造计时框架。

## 分层（禁止互相冒充）

Living `15` / `18`：

| layer | 典型 GC | 测什么 |
|-------|---------|--------|
| `kernel` | Disabled / n/a | limb 极限、栈上 floor |
| `numeric` | 隔离 heap · Disabled/Deferred · 复用 ctx | 分配 / publish / borrowed `Integer` 算术 |
| `e2e` | `shared_default` · Auto | 公共便利路径（含 owning clone / 每调用 context） |
| `peer` | n/a | `num-bigint` / `ibig` / `malachite` |

`Integer::try_add` 在 **session Disabled** 与 **shared Auto** 下数字差一个数量级时，不得把 e2e 写成 kernel 结论。

## 分组（`athena-bench`）

| 组 | 内容 |
|----|------|
| `path` | 所有权 / GC 路径分段（4 limb ≈256-bit） |
| `numeric` | ExactInteger / ExactRational / promotion |
| `bigint` | 统一矩阵（Athena layers + optional peers；默认 suite **不**eager 注册） |
| `ir` / `rewriter` / `engine` / `domains` / `jit` / `infra` | 各域种子 |

```sh
cargo run -p athena-benchmark --release --bin athena-bench -- --groups path --format text
cargo run -p athena-benchmark --release --bin athena-bench -- --groups path --format markdown --write path.md
cargo run -p athena-benchmark --release --bin athena-bench -- --groups bigint --format markdown --write bigint.md
```

Path text 输出的 `ns/op` 已按 `PATH_BATCH`（2000）归一化。

## Path 最近一次成绩（Living `18` A 后）

- 机器：Windows · `athena-bench --release --groups path`
- 日期：2026-09-02
- 规模：4×`u64` limb（≈256-bit）

| id | layer | gc | ns/op | 说明 |
|----|-------|-----|------:|------|
| `path.stack_add_4` | kernel | n/a | ~1 | 栈上 floor |
| `path.alloc_numeric_block_4` | numeric | disabled | ~70–80 | alloc+release |
| `path.publish_from_limbs_4` | numeric | disabled | ~130 | `from_limbs_in` |
| `path.clone_heap_natural_4` | numeric | disabled | ~90 | Natural owning clone |
| `path.natural_try_add_4` | numeric | disabled | ~240 | Natural 基线 |
| `path.integer_try_add_session_4` | numeric | disabled | **~250** | borrowed view + session 发布 |
| `path.integer_try_add_4` | e2e | auto | **~4300** | 同算法，shared Auto 发布 |
| `path.integer_clone_4` | e2e | auto | ~4000 | owning Heap clone |
| `path.integer_add_e2e_4` | e2e | auto | ~4500 | 每调用便利 `add` |

进程退出偶发 `ACCESS_VIOLATION` 是独立生命周期缺陷，不与算术性能混修。

## Criterion：`compare-bigint`

| Feature | 对照库 |
|---------|--------|
| `compare-num-bigint` | `num-bigint` |
| `compare-ibig` | `ibig` |
| `compare-malachite` | `malachite` |
| `compare-bigint` | 以上全部 |

```sh
cargo bench -p athena-benchmark --features compare-bigint --bench compare_bigint
```

Athena 热路径在迭代外复用同一个 `NumericContext`，调用 `try_*`。这对应 **numeric / reused** 层，**不是** e2e `Integer::add` 每调用建 context。

对照数字请用当前 Criterion HTML / `athena-bench --groups bigint` Markdown；下文旧表（含每次重建 context 的测量）**作废，勿引用**。

外部 bigint 依赖仅挂在本 crate 的可选 `compare-*` feature 上，**不得**回流到 `athena-types` / `athena-numeric` / `athena-engine`。
