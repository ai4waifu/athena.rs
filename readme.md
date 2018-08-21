# Athena

Athena 是 SXO 背后的纯 Rust 计算机代数内核，代码仓库位于 `euler.rs`（远程 `athena.rs`）。它负责数学语义、Core IR、求值、重写与会话状态，并与
Node.js、N-API、WebAssembly 绑定及语言前端保持独立。

本文档面向希望参与内核建设的贡献者。方言解析、降级和渲染属于 SXO 前端，Athena 不解析 Mathematica 或 MATLAB，也不承载宿主平台集成。

## 代码分层

```text
SXO 方言前端 → Athena IR / value → athena（门面）→ athena-engine → result / diagnostic
```

| Crate | 主要职责 |
|-------|----------|
| [`athena-types`](projects/athena-types/readme.md) | 数值表示、标识符、源码位置和诊断合同 |
| [`athena-ir`](projects/athena-ir/readme.md) | arena 管理的 Core CAS IR、构建器、验证和哈希 |
| [`athena-rewriter`](projects/athena-rewriter/readme.md) | 规范化与重写基础设施 |
| [`athena-engine`](projects/athena-engine/readme.md) | **唯一执行引擎**：Session、求值、领域编排 |
| [`athena`](projects/athena/readme.md) | **薄公共门面**：稳定 re-export 与构造入口 |

`athena-engine` 是执行实现边界；`athena` 是公开兼容边界。依赖只能是 `athena → athena-engine`。多项式、矩阵、数论和图算法等数学主题应作为 `athena-engine` 模块演进，不应拆成一组微型 crate。

## 当前阶段

项目仍在先稳定公共类型合同与 crate 边界、再扩展求值和领域能力的阶段。目录中出现的类型或模块不等于功能已经生产就绪。提交新语义时，请明确它增加或维护了哪些不变量，并用针对性测试证明行为。

## 开发与验证

需要稳定版 Rust 工具链和 Cargo。

```sh
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

迭代单个 crate 时可运行 `cargo test -p <crate>`。

## 贡献约定

- 先阅读类型合同和 IR 验证逻辑，再添加新的数学语义。
- 保持诊断语言无关：提供稳定 code、结构化参数和 source span，文案交给 SXO 等宿主前端。
- 不在本仓库加入方言解析器、平台绑定或 JavaScript 集成。
- 只有依赖关系和发布生命周期确实独立时才新增 crate，否则优先新增 `athena-engine` 模块。
- 修改求值、重写或对象语义时，同时覆盖正常结果、边界输入和错误路径。
- 默认保持纯 Rust（含 wasm32）；不得让门面或 default feature 链接 MKL/BLAS。

## 许可证

MPL-2.0，见 [`License.md`](License.md)。
