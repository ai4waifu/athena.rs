# `athena-engine`

`athena-engine` 是 Athena 的 **唯一执行引擎**。它决定数学表达式「怎么算」，不知道用户「怎么写」或结果「怎么显示」。

## 职责

- Core IR 求值与数值 promotion
- `Session`、符号绑定与作用域
- M-Graph 状态 / 闭包与 solver reflector 协议（骨架）
- 化简 / 重写流水线编排（调用 `athena-rewriter`）
- 微分、积分及其他领域操作编排
- 资源 / 递归限制与取消检查
- `ATHENA_*` 稳定诊断
- Runtime object 与数学结果

## 不负责

- 任何源语言 parse、文本字面量解析或前端 AST 转换
- 字符串 render、locale
- N-API / WASM / JS handle 生命周期
- 公开兼容边界（由 [`athena`](../athena/readme.md) 负责）

Athena 只接受 **已经构造**的 `Number` / IR / runtime `Term`；不得从源文本构造值。

## 与 `athena` 的边界

| Crate           | 边界                                       |
|-----------------|--------------------------------------------|
| `athena-engine` | 执行实现边界                               |
| `athena`        | 公开兼容边界（薄门面，re-export 本 crate） |

依赖方向只能是 `athena → athena-engine`。禁止反向依赖。不得把「单 crate 改名」当成两者拆分已完成。

默认 feature 为纯 Rust（含 `wasm32`）。不得在 default feature 中链接 MKL/BLAS。

## M-Graph 与 Galois connection

M-Graph 是建立在 AthenaIR（理论别称 MSM）之上的 typed mathematical fact graph。它把表达式、代数对象和域之间的关系记录为带作用域、保证级别、证据和依赖的 claims。只有经过 verifier 接受的无条件精确事实，才能进入等价闭包并驱动重写。概率、近似、假设依赖和资源截断结果不会被伪装成 exact。

其抽象解释基础是具体语义域 `C` 与抽象事实域 `A` 之间的 Galois connection：

```text
α(c) ⊑ a  当且仅当  c ⊑ γ(a)
```

抽象映射 `α` 提取可传播的 facts，具体化映射 `γ` 描述 facts 允许的具体状态。这个关系约束 transfer 和 verifier 的 soundness，但不替代证书检查。

M-Graph 的 verified 子图可以提取为 KernelIR 执行计划，再经过 guard 进入 JIT。JIT 不可用或 guard 失败时回退 eager 执行，并保持数学语义不变。

```sh
cargo test -p athena-engine
cargo doc -p athena-engine --no-deps
```
