# `athena`

`athena` 是 Athena 计算机代数内核的公共 Rust 门面。它将 workspace 合同、Core IR 和重写器组合成统一的数学接口。

本 crate 负责数值 promotion、运行时对象、符号与绑定、领域分派、求值、化简、微分、积分和语言无关的 `ATHENA_*` 诊断。数值、矩阵、多项式、数论和图相关数学属于同一数学内核，不应拆成 `athena-num`、`athena-eval` 等微型产品。

## 与 `athena-engine` 的边界

`athena` 与 `athena-engine` 是两个不同的 crate。`athena` 是稳定公共门面，`athena-engine` 是独立的执行引擎层；不得通过重命名 `athena` 来代替新增 `athena-engine`，也不得在两者之间维护两套数学语义。

## 与 SXO 的边界

Athena 不解析 Mathematica 或 MATLAB，也不提供 N-API 或 WebAssembly 绑定。SXO 负责解析、方言 lowering、渲染、locale 选择和宿主集成。SXO 适配器不得重新实现 Athena 的数值、对象或求值语义。

```text
SXO 前端 → Athena IR/value → Athena 公共门面 → result/diagnostic
```

```sh
cargo test -p athena
cargo doc -p athena --no-deps
```
