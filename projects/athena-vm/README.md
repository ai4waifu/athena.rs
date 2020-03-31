# `athena-vm`

`athena-vm` 是 Athena 受限的 typed `ExecutionIR` 运行时：frame / slot / budget / cancellation / safepoint / 解释执行。

## 职责

- `VmModule` / `Instruction` / `VmConfig` / `VmExit` / `Interpreter`
- 稠密 `SlotTable` / `SlotValue`（仅 typed 句柄）与 `Frame` / `FrameStack`
- 协作式 `CancellationToken` 与步数预算（在 safepoint / 步进处检查）

## 非职责

不解析方言、不做 Mathematica 名称分派、不拥有 M-Graph admission、不拥有持久数学 payload、不自建 GC heap。  
不算第二个 `athena-engine`。

## 依赖

```text
athena-types → athena-gc → … → athena-rewriter → athena-vm → athena-engine → athena
```

## 验证

```sh
cargo test -p athena-vm --test main
cargo doc -p athena-vm --no-deps
```
