# `athena`

`athena` 是 Athena 计算机代数内核的**稳定公共 Rust 门面**。它很薄：re-export `athena-engine` 与必要的 IR/types 合同，并控制公开 API 面积。

## 职责

- 稳定公共入口（如 `AthenaEngine`、`Session`、常用类型）
- 必要的 re-export 与 feature 汇总
- 对外兼容边界（普通 Rust 使用者只依赖本 crate）

## 不负责

- 再定义一套 Engine / Session / eval
- 方言特判、oak、N-API、WASM
- 在 facade 中复制数学语义

## 与 `athena-engine` 的边界

| Crate | 边界 |
|-------|------|
| `athena-engine` | 执行实现边界（怎么算） |
| `athena` | 公开兼容边界（怎么接） |

依赖方向只能是 `athena → athena-engine`。禁止反向。不得通过重命名单 crate 代替拆分。

## 与 SXO 的边界

Athena 不解析 Mathematica 或 MATLAB，也不提供 N-API 或 WebAssembly 绑定。SXO 宿主（`sxo-napi` / `sxo-lite-wasm`）应依赖 **`athena`**；方言 crate 只依赖 `athena-ir` / `athena-types`。

```text
SXO 前端 → Athena IR/value → athena（门面）→ athena-engine → result/diagnostic
```

```sh
cargo test -p athena
cargo doc -p athena --no-deps
```
