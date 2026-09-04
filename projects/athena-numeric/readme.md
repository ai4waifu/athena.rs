# athena-numeric

Athena 数值塔：表示、精度、promotion、比较、证书与序列化。

- **值与存储**：`NumericValue` · `Natural`/`Integer`（24B `meta + Magnitude`）· `Rational`（48B 双 Magnitude）
- **正确性门**：N0–N3 已关闭。N4：能力三分 · `KernelTable` · `foreign::mpn_oracle` 差分 · destination reuse / `*_owned`
- **N5**：ANV1 多 kind encode/decode + reject 矩阵与 LCG fuzz 已接线。Heap 经 `athena-gc` 分配

`num-*` 不出现在 workspace 根依赖。可选外部对照见 `athena-benchmark`（`compare-*`）。依赖：`athena-types` · `athena-gc`。
