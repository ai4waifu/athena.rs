# athena-numeric

Athena 数值塔：表示、精度、promotion、比较、证书与序列化。

- **值与存储**：`NumericValue` · `Natural`/`Integer`（24B `meta + Magnitude`）· `Rational`（48B 双 Magnitude）
- **正确性门**：N0–N3 已关闭。N4 进行中：能力三分 · `KernelTable` context 绑定 · `NumericExecutor` · `algorithm/` 策略
- **N5**：ANV1 Int/Rat reject 已接线。Heap 经 `athena-gc` 分配

`num-*` 不出现在 workspace 根依赖。可选外部对照见 `athena-benchmark`（`compare-*`）。依赖：`athena-types` · `athena-gc`。
