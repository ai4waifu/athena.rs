# `athena-types`

`athena-types` 是 Athena workspace 共享的轻依赖合同 crate。它存放在 IR、重写器和内核各模块之间必须保持唯一含义的数据结构。

## 职责范围

- 稳定 ID（`TermId`、`NodeId`、`SymbolId`、`OperatorId`、`DomainId`）与轻量 `NumericKind`。
- 过渡期 `Number` / `ExactNumber` / `RealNumber`（算术实现将迁入 `athena-numeric`）。
- 精度与舍入策略类型。
- source span 与语言无关诊断。
- 序列化和版本标记。

本 crate 定义合同，不实现求值算法。它不依赖 parser、session、重写引擎、前端 AST、N-API 或 WebAssembly。

修改公共类型会影响所有 Athena crate 和外部调用方。优先使用结构化字段和显式枚举，不要用字符串承载语义，也不得通过隐式机器浮点转换丢失精确性。

```sh
cargo test -p athena-types
cargo doc -p athena-types --no-deps
```
