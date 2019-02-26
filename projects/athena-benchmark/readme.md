# `athena-benchmark`

测量与校验 Athena 内核在固定 fixture 上的行为。基准结果不定义数学语义或稳定 API；正确性校验优先于计时。

**不进主 CI。** 仅本地或单独手动流程运行。Mathematica / MATLAB 等外部对照属于 SXO 本地 opt-in（`bench:local`），不是本 crate 的职责。

## 两套入口（分工冻结）

| 入口 | 工具 | 用途 |
|------|------|------|
| `athena-bench` | 自有 fixture + JSON/Markdown | **合同校验** + GC/arena/scratch 资源采样（**不计 ns/op**） |
| `cargo bench` | **Criterion** | **唯一**性能计时（warmup / 自动 iteration / 统计 / HTML） |

```text
athena-benchmark
├── fixture / validation / report schema   ← athena-bench
└── Criterion benches                      ← 唯一 ns/op
    ├── path_segments
    ├── compare_bigint（feature 门控 peers）
    └── …
```

禁止在本 crate 内用 `Instant::now()` 自造微基准计时器，也禁止把 `athena-bench` 报告里的字段当成算法 ns/op。

二者必须共享：fixture · operation · layer · context policy · gc 标注。不得各写一套操作语义。

## 分层（禁止互相冒充）

Living `15` / `18`：

| layer | 典型 GC | 测什么 |
|-------|---------|--------|
| `kernel` | Disabled / n/a | limb 极限、栈上 floor |
| `numeric` | 隔离 heap · Disabled/Deferred · 复用 ctx | 分配 / publish / borrowed `Integer` 算术 |
| `e2e` | `shared_default` · Auto | 公共便利路径（含 owning clone / 每调用 context） |
| `peer` | n/a | `num-bigint` / `ibig` / `malachite` |

`Integer::try_add` 在 **session Disabled** 与 **shared Auto** 下数字差一个数量级时，不得把 e2e 写成 kernel 结论。

## 命令

```sh
# 合同 / 资源（无 ns/op）
cargo run -p athena-benchmark --release --bin athena-bench -- --groups path --format text
cargo run -p athena-benchmark --release --features compare-bigint --bin athena-bench -- --groups bigint --format markdown --write target/bigint-contract.md

# 性能（Criterion）
cargo bench -p athena-benchmark --bench path_segments
cargo bench -p athena-benchmark --features compare-bigint --bench compare_bigint
```

## Path / bigint 读数说明

旧 `athena-bench` 单次 `Instant` 或固定 `PATH_BATCH` 归一化数字（含 readme 历史摘录）**不得**再当作精确 ns/op。可信性能结论必须来自 Criterion 重测后再写入。

已知方向性结论（仍须 Criterion 钉死倍率）：

- e2e shared Auto add@256 ≈ 数微秒级，是 GC/publish，不是 limb 算法
- numeric session Disabled 已把 Integer add 拉到与 Natural 同量级（约数百 ns 量级）
- 旧表里 64–256 bit 的 100/200/300 ns 台阶多为分辨率噪声

## Criterion：`compare-bigint`

| Feature | 对照库 |
|---------|--------|
| `compare-num-bigint` | `num-bigint` |
| `compare-ibig` | `ibig` |
| `compare-malachite` | `malachite` |
| `compare-bigint` | 以上全部 |

Athena 热路径在迭代外复用同一个 `NumericContext`，调用 `try_*`。这对应 **numeric / reused** 层，**不是** e2e `Integer::add` 每调用建 context。

外部 bigint 依赖仅挂在本 crate 的可选 `compare-*` feature 上，**不得**回流到 `athena-types` / `athena-numeric` / `athena-engine`。
