# athena-numeric

Athena 数值塔：表示、精度、promotion、比较、证书与序列化（Living `16` N0–N2）。

- N0：`NumericValue` / backend trait / 模块骨架
- N1：`ExactInteger` / `ExactRational` · normalize · gcd · sign · compare · serialize
- N2：Integer↔Rational · Exact↔Machine · Machine↔Arbitrary · mismatch 诊断（migration gate：`tests/numeric_promotion_n2.rs`）

`num-*` 仅作内部存储，不作为公共语义。依赖：`athena-types`。
