# `athena-jit`

可选 native JIT 加速层：编译已验证的 `ExecutionIR`，guard 失败时走声明的 deopt / partial 出口，**不改变** exact / promotion / 诊断语义。

默认 **关闭**（`Cargo.toml` 无默认 feature）。`wasm32` 目标须报告 `UnsupportedTarget`。

```sh
cargo test -p athena-jit
cargo doc -p athena-jit --no-deps
```
