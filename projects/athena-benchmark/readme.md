# `athena-benchmark`

测量与校验 Athena 内核在固定 fixture 上的行为。基准结果不定义数学语义或稳定 API；正确性校验优先于计时。

**不进主 CI。** 仅本地或单独手动流程运行。

## 两套入口（分工冻结）

| 入口           | 工具                         | 用途                                                       |
|----------------|------------------------------|------------------------------------------------------------|
| `athena-bench` | 自有 fixture + JSON/Markdown | **合同校验** + GC/arena/scratch 资源采样（**不计 ns/op**） |
| `cargo bench`  | **Criterion**                | **唯一**性能计时                                           |

禁止在本 crate 内用 `Instant::now()` 自造微基准。Criterion 长迭代会打满默认 256 MiB bump arena，因此 Athena 侧微基准使用
`HeapBudget::for_microbench()`（仍强制预算检查，只抬高上限）。

## 分层

| layer     | 典型 GC             | 测什么                              |
|-----------|---------------------|-------------------------------------|
| `kernel`  | Suspend/Disabled    | Natural / limb 路径                 |
| `numeric` | Deferred · 复用 ctx | borrowed `Integer` + session 发布   |
| `e2e`     | shared Auto         | 公共便利 `add` / owning clone       |
| `peer`    | n/a                 | `num-bigint` / `ibig` / `malachite` |

## 命令

```sh
cargo run -p athena-benchmark --release --bin athena-bench -- --groups path --format text
cargo bench -p athena-benchmark --bench path_segments
cargo bench -p athena-benchmark --features compare-bigint --bench compare_bigint
```

## Criterion 读数（2026-09-03 · Windows · commit 附近 `6a26588`+）

测量：`--warm-up-time 1 --measurement-time 2..3 --sample-size 40..50`。下表为 Criterion 估计中心值（约数）。

### Path（4 limb ≈256-bit）

| case                                 |     time |
|--------------------------------------|---------:|
| `stack_add_4`                        | ≈ 3.7 ns |
| `natural_try_add` · Disabled         | ≈ 2.6 µs |
| `integer_try_add` · session Deferred | （本切片改为 Deferred · 待复测） |
| `integer_try_add` · shared Auto      | ≈ 6.0 µs |
| `integer_add` e2e                    | ≈ 5.7 µs |
| `integer_clone` · shared Auto        | ≈ 5.6 µs |

### `bigint_add`（athena / peer）

| bits | athena kernel | athena numeric | athena e2e |      num |     ibig | malachite |
|-----:|--------------:|---------------:|-----------:|---------:|---------:|----------:|
|   64 |       ≈ 14 ns |        ≈ 49 ns |   ≈ 196 ns |  ≈ 84 ns |  ≈ 80 ns |   ≈ 76 ns |
|  256 |      ≈ 2.2 µs |       ≈ 2.2 µs |   ≈ 7.6 µs | ≈ 188 ns | ≈ 100 ns |  ≈ 256 ns |
| 1024 |      ≈ 3.4 µs |       ≈ 2.7 µs |   ≈ 5.9 µs | ≈ 200 ns | ≈ 103 ns |  ≈ 192 ns |
| 4096 |      ≈ 6.8 µs |       ≈ 6.7 µs |   ≈ 6.0 µs | ≈ 244 ns | ≈ 157 ns |  ≈ 250 ns |

相对最快 peer（通常 `ibig`）的 **numeric** 倍率（athena / peer，>1 = 更慢）：

| bits |             vs ibig |
|-----:|--------------------:|
|   64 | **≈ 0.61×**（更快） |
|  256 |           **≈ 22×** |
| 1024 |           **≈ 26×** |
| 4096 |           **≈ 43×** |

### 结论（取代旧 Instant 表）

- 旧 `athena-bench` 单次 `Instant` / 固定 batch 表（100/200/300 ns 台阶、声称 add 仅 1.5–3×） **作废**。
- **64-bit**：Athena numeric 可与 peer 打平或更快（inline 小整数路径）。
- **≥256-bit heap 发布路径**：Athena numeric/kernel ≈ **2–7 µs**，peer ≈ **100–250 ns**，差距是 **数量级**，不是
  1.5–3×。主导成本是 value/context/publish（bump arena 结果发布），不是「add kernel 慢 2×」。
- **e2e shared Auto** 仍约 **6–8 µs**（clone / 每调用便利路径），与 Living 18 债务一致。
- 下一步优化应针对 **heap publish / 结果构造**，不要先改 add limb 算法。

## Criterion features

| Feature              | 对照库       |
|----------------------|--------------|
| `compare-num-bigint` | `num-bigint` |
| `compare-ibig`       | `ibig`       |
| `compare-malachite`  | `malachite`  |
| `compare-bigint`     | 以上全部     |

外部 bigint 依赖不得回流到核心 `athena-*` crates。
