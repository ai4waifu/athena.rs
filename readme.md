# Athena

[![Rust CI](https://github.com/ai4waifu/athena.rs/actions/workflows/rust.yml/badge.svg)](https://github.com/ai4waifu/athena.rs/actions/workflows/rust.yml)

Athena 是纯 Rust 计算机代数内核，为 Rust 程序提供精确数值、符号表示、重写与数学领域计算。计算通过统一的会话和执行入口提交，返回带有状态、条件、证据和诊断的结构化结果。

项目正在开发中，各领域的实现程度不同。使用具体能力前，请查阅对应模块的 API 和测试，确认支持范围与边界行为。

## 接入

Rust 调用方通过 [`athena`](projects/athena/readme.md) crate 使用公共 API，将输入构造成数值、符号项和中性计算请求。源语言解析、渲染与平台适配由调用方或前端负责。

当前 `athena` crate 尚未发布到 crates.io，可在本地检出仓库后通过路径依赖接入：

```toml
[dependencies]
athena = { path = "../athena.rs/projects/athena" }
```

路径相对于调用项目的 `Cargo.toml`，请按实际目录调整。仓库使用 nightly Rust，工具链配置见 [`rust-toolchain.toml`](rust-toolchain.toml)。

- [公共入口](projects/athena/readme.md)：`AthenaEngine`、`Session` 与公共类型。
- [执行引擎](projects/athena-engine/readme.md)：请求、会话、结果与领域计算。
- [符号表示](projects/athena-ir/readme.md)：`TermStore`、符号项构建与验证。

在仓库根目录生成并打开 API 文档：

```sh
cargo doc -p athena --no-deps --open
```

## 模块导航

Athena 以 `TermStore` 保存符号结构，以 M-Graph 组织语义关系和计算过程。领域算法提供计算能力，重写器提供候选变换，验证与准入过程决定哪些关系可以成为可信事实。结果分别表达精确性、成立条件、覆盖范围和资源限制。

核心模块：

- [`athena-types`](projects/athena-types/readme.md)：共享身份、状态与诊断。
- [`athena-gc`](projects/athena-gc/readme.md)：运行时堆、对象生命周期与资源预算。
- [`athena-numeric`](projects/athena-numeric/readme.md)：数值表示、精度与算术内核。
- [`athena-ir`](projects/athena-ir/readme.md)：符号项、结构共享与中间表示。
- [`athena-rewriter`](projects/athena-rewriter/readme.md)：规则匹配、替换与重写候选。
- [`athena-engine`](projects/athena-engine/readme.md)：会话、M-Graph、执行与数学领域算法。
- [`athena`](projects/athena/readme.md)：对外 Rust API。

数据与开发工具：

- [`athena-graph`](projects/athena-graph/readme.md)：离散图存储、视图与基础算法。
- [`athena-ndarray`](projects/athena-ndarray/readme.md)：多维数组与存储。
- [`athena-table`](projects/athena-table/readme.md)：列式表、schema 与查询合同。
- [`athena-testing`](projects/athena-testing)：共享测试工具。
- [`athena-benchmark`](projects/athena-benchmark/readme.md)：性能与资源基准。

## 本地开发

在仓库根目录执行：

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --features athena-numeric/ephemeral
cargo clippy --workspace --all-targets -- -D warnings
```

测试命令与 [CI 配置](.github/workflows/rust.yml) 保持一致。性能基准的运行方式见 [`athena-benchmark`](projects/athena-benchmark/readme.md)。

## 许可证

Apache-2.0，见 [`License.md`](License.md)。
