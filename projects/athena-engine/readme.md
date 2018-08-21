# `athena-engine`

`athena-engine` 是 Athena 的**唯一执行引擎**。它决定数学表达式「怎么算」，不知道用户「怎么写」或结果「怎么显示」。

## 职责

- Core IR 求值与数值 promotion
- `Session`、符号绑定与作用域
- 化简 / 重写流水线编排（调用 `athena-rewriter`）
- 微分、积分及其他领域操作编排
- 资源 / 递归限制与取消检查
- `ATHENA_*` 稳定诊断
- Runtime object 与数学结果

## 不负责

- Mathematica / MATLAB 等语法解析、`WExpr`、oak
- 方言 profile、字符串 render、locale
- N-API / WASM / JS handle 生命周期
- 对外产品兼容边界（见 [`athena`](../athena/readme.md)）

## 与 `athena` 的边界

| Crate | 边界 |
|-------|------|
| `athena-engine` | 执行实现边界 |
| `athena` | 公开兼容边界（薄门面，re-export 本 crate） |

依赖方向只能是 `athena → athena-engine`。禁止反向依赖。不得把「单 crate 改名」当成两者拆分已完成。

默认 feature 为纯 Rust（含 `wasm32`）。不得在 default feature 中链接 MKL/BLAS。

```sh
cargo test -p athena-engine
cargo doc -p athena-engine --no-deps
```
