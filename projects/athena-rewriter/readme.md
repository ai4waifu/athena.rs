# `athena-rewriter`

`athena-rewriter` 为 Athena Core CAS IR 提供可复用的变换。它与内核门面分离，使重写行为可以独立测试和组合。

职责包括规范化、规则应用、化简结果和重写诊断，同时维护 IR 与源码不变量。它不解析前端语言、不拥有 session、不渲染方言，也不选择
locale 文案。重写不得静默改变精确性或数值策略。

主要 API 为 `Rewriter`、`RewriteOptions`、`RewriteResult` 和 `Rewriter::simplify`。

```sh
cargo test -p athena-rewriter
cargo doc -p athena-rewriter --no-deps
```
