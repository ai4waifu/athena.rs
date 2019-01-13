# athena-numeric

Athena 数值塔：表示、精度、promotion、比较、证书与序列化。

- **核心表示**：`NumericValue`、backend trait、模运算与 limb 内核
- **精确算术**：`ExactInteger` / `ExactRational` · normalize · gcd · sign · compare · serialize
- **promotion**：Integer↔Rational · Exact↔Machine · Machine↔Arbitrary · mismatch 诊断（见 `tests/exact/promotion.rs`；CI：`cargo test -p athena-numeric --test main promotion`）

`num-*` 不出现在 workspace 根依赖；核心 crate 零 `num-*`。可选外部对照仅见 `athena-benchmark`（`bigint-compare` feature）。依赖：`athena-types`。
