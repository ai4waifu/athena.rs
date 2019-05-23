# `athena-ir`

`athena-ir` 负责 Athena 的 Core CAS 表示。各前端将自身语法 lowering 到此表示，再由内核进行求值和重写。

IR 基于 arena：term 通过稳定 ID 引用子项，而不是递归拥有所有子项。这使结构共享、确定性哈希、验证和增量重写成为可能。

本 crate 提供 `TermStore`、`TermNode`、`Atom`、`TermBuilder`、`SymbolTable`、`canonical_hash` 和 `TermStore::verify`。依赖
`athena-numeric`（数字原子已持有 `NumericValue`）。

IR 与具体输入语法无关。源文本 AST 和界面层对象不属于这里。新增节点必须补充构造、验证、哈希测试，并在适用时补充序列化测试。

```sh
cargo test -p athena-ir
cargo doc -p athena-ir --no-deps
```
