# `athena-testing`

Athena 中立测试辅助库：给其他 crate 的 `cargo test` 提供构造器与类型化断言。

## 职责

- `TermBuilder` / `SessionFixture` / `DomainRequestBuilder`：中立请求与会话夹具
- 类型化断言（结构相等、精确整数、诊断码）
- M-Graph / rewrite / execution / lifecycle 测试辅助 API

## 非职责

- 不是独立测试二进制
- 不是 `@sxo/harness` 或方言表面
- 不实现 CAS 语义内核

## 依赖

```text
athena-types → … → athena-engine（按需）→ athena-testing
```

仅被测试代码依赖。生产路径不要依赖本 crate。

## 验证

```sh
cargo test -p athena-testing
cargo doc -p athena-testing --no-deps
```
