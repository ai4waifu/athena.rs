# `athena-benchmark`

`athena-benchmark` 用来检查 Athena 的数值与执行路径是否正确，并测量典型操作在不同输入规模下的耗时和资源开销。

## 快速开始

```sh
cargo run -p athena-benchmark --release --bin athena-bench -- --groups bigint --format text
cargo bench -p athena-benchmark --bench allocation_modes
cargo bench -p athena-benchmark --bench path_segments
cargo bench -p athena-benchmark --features compare-bigint --bench compare_bigint
```

`athena-bench` 支持 `--format text` 和 `--format json`。使用 `--help` 查看全部 fixture 分组和参数。

## 性能读数

以下是 Windows 上的近期参考结果，Criterion 使用短预热和约 1 秒测量时间。读数用于看数量级和回归方向，不是跨机器的固定承诺。

### 分配路径

| 操作                   |     耗时 | 说明             |
|------------------------|---------:|------------------|
| 栈上 4-limb 加法       | ≈ 3.9 ns | 纯底层运算       |
| raw bump 32 B          |  ≈ 48 ns | 仅推进分配游标   |
| batch 分配 4 limbs     |  ≈ 52 ns | `NumericBatch`   |
| 256-bit ephemeral 加法 |  ≈ 99 ns | 批处理内加法     |
| 4-limb `promote`       | ≈ 750 ns | 跨堆发布结果     |
| Full 分配并释放        | ≈ 1.5 µs | 完整对象生命周期 |

### 大整数加法 `bigint_add`

|  位宽 | Athena numeric |   `ibig` | Athena e2e |
|------:|---------------:|---------:|-----------:|
|    64 |       ≈ 120 ns |  ≈ 84 ns |   ≈ 384 ns |
|   256 |       ≈ 136 ns |  ≈ 93 ns |   ≈ 5.9 µs |
|  1024 |       ≈ 194 ns |  ≈ 97 ns |   ≈ 5.5 µs |
|  4096 |       ≈ 400 ns | ≈ 145 ns |   ≈ 5.5 µs |
| 16384 |       ≈ 984 ns | ≈ 376 ns |   ≈ 5.7 µs |

### 大整数乘法 `bigint_mul`

|  位宽 | Athena kernel | Athena numeric |   `ibig` | Athena e2e |
|------:|--------------:|---------------:|---------:|-----------:|
|  4096 |      ≈ 7.3 µs |       ≈ 8.1 µs | ≈ 4.7 µs |    ≈ 13 µs |
| 16384 |       ≈ 66 µs |       ≈ 121 µs |  ≈ 47 µs |    ≈ 74 µs |

加法的 numeric 路径与 `ibig` 处于同一数量级。公共 `e2e` API 还包含上下文和 owning 结果成本。大规模乘法中，kernel
使用更快的乘法算法，numeric 临时路径仍有优化空间。

## 运行与解读

`athena-bench` 输出正确性结果和资源采样。`cargo bench` 输出 Criterion 报告，详细 HTML 位于 `target/criterion/`。报告按
`kernel`、`numeric`、`e2e` 和 `peer` 层次展示，请只比较同一层次的数据。

可选对照库 feature：`compare-num-bigint`、`compare-ibig`、`compare-malachite`，或一次启用全部库的 `compare-bigint`。
