# Athena

Athena 是一个纯 Rust 计算机代数内核。它把数值、符号表达式和计算请求转换为可验证的 Core IR，在统一的求值、重写和会话模型中执行，并返回结构化结果或诊断。

Athena 适合需要精确数值、符号变换、代数化简、微积分和领域算法的 Rust 程序。它不解析源语言文本，不包含命令行界面，不持有设备或平台对象。调用方负责把输入构造成
Athena 的类型和 IR，再通过公共门面提交计算。

## 能力边界

Athena 内核负责：

- 精确整数、有理数及其它数值域的表示、精度和 promotion。
- Runtime heap 管理的 Core IR 与数值 block、稳定 ID、结构共享、验证和确定性哈希。
- 求值、符号绑定、作用域、会话状态和资源限制。
- 规范化、规则重写、化简以及可组合的 rewrite pipeline。
- 微分、积分、级数、变换、数论和其它领域算法。
- 语言无关的结构化诊断和可取消的计算流程。

内核不负责源文本解析、语法树、渲染、用户界面、平台绑定或应用工作流。任何外部输入都必须先转换为 `athena-types` 和 `athena-ir`
能理解的值或节点。

## 计算路径

```text
构造 Number / Core IR
        ↓
athena（稳定公共门面）
        ↓
athena-engine（求值、重写、Session、领域算法）
        ↓
RuntimeValue / Diagnostic
```

`athena` 负责公开 API 和兼容边界，`athena-engine` 负责执行实现。依赖方向只能是 `athena → athena-engine`，不得反向依赖，也不得在
facade 中复制另一套求值或会话语义。

## Crate 分层

| Crate                                                     | 作用                                                   |
|-----------------------------------------------------------|--------------------------------------------------------|
| [`athena-types`](projects/athena-types/readme.md)         | 共享 ID、数值元数据、诊断、span 和版本合同             |
| [`athena-gc`](projects/athena-gc/readme.md)               | CAS runtime GC heap：segmented arena、tracing、scratch、`GcMode` |
| [`athena-numeric`](projects/athena-numeric/readme.md)     | 数值塔、精度、promotion 和数值证书                     |
| [`athena-ir`](projects/athena-ir/readme.md)               | Core IR、构建器、验证和哈希（对象存储终局归属 runtime heap） |
| [`athena-rewriter`](projects/athena-rewriter/readme.md)   | 规范化、规则匹配、重写结果和重写诊断                   |
| [`athena-engine`](projects/athena-engine/readme.md)       | 唯一执行引擎，包含 Session、M-Graph、solver 和领域编排 |
| [`athena`](projects/athena/readme.md)                     | 薄公共门面和稳定 re-export                             |
| [`athena-ndarray`](projects/athena-ndarray/readme.md)     | CAS N 维数组与 out-of-core storage                     |
| [`athena-graph`](projects/athena-graph/readme.md)         | 普通离散图与大图算法基座（非 M-Graph）                 |
| [`athena-table`](projects/athena-table/readme.md)         | 列式表、schema 与惰性查询合同（非 ML）                 |
| [`athena-benchmark`](projects/athena-benchmark/readme.md) | 固定输入集上的性能和资源基准                           |

依赖方向：`types → gc → numeric → ir → rewriter → engine → athena`。数学主题应作为已有 crate 中职责清晰的模块演进。除非依赖关系、版本生命周期和测试边界确实独立，否则不要新增微型 crate。`athena-gc` 是运行时基础层，不是领域微 crate；禁止再拆 `athena-arena`。M-Graph
和 solver 继续归属于 `athena-engine`，不另建同名 crate。

## 当前状态

基础数值合同、Core IR
arena、重写器、公共门面以及多个求值和领域模块已经存在。部分领域能力仍处于逐步扩展阶段，代码中出现的类型或模块不代表已经具备稳定的生产兼容承诺。判断一项能力是否可用，应以对应测试、边界行为和错误路径为准。

## 开发与验证

需要 Rust 工具链和 Cargo。常用验证命令：

```sh
cargo test --workspace
cargo test -p athena-numeric --test main promotion
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --summary-only
cargo run -p athena-benchmark --release -- --groups numeric,ir --json
```

迭代单个 crate：

```sh
cargo test -p athena-engine --test main
cargo test -p athena-numeric --test main
cargo test -p athena-ir
cargo doc -p athena --no-deps
```

行覆盖率用 `cargo llvm-cov` 汇报（CI 上传 artifact 与 job summary），不设百分比失败门槛。测试布局见 Living「测试与验收」：每
crate `tests/main.rs` + 域 `mod.rs`。

修改数值、IR、求值、重写或对象语义时，应同时覆盖正常结果、边界输入、资源限制和结构化错误路径。默认保持纯 Rust，并确保 `wasm32`
构建不依赖系统 GPU、MKL 或 BLAS。

## 诊断与数值原则

诊断使用稳定 code、结构化参数、详情和 source span，不在内核中写面向用户界面的自然语言文案。数值转换必须显式表达精度和
promotion，禁止通过隐式机器浮点转换丢失精确性。

## 许可证

Apache-2.0，见 [`License.md`](License.md)。
