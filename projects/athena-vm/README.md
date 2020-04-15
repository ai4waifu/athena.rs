# `athena-vm`

`athena-vm` 是 Athena 的 **ExecutionIR 执行运行时**（怎么跑已编译 IR）。

`athena-engine` 是其**之上**的综合体（规划 · 编译 · Session · M-Graph · 域 · AdmissionGate），经依赖挂在 `athena-vm` 之上，**不是**并列第二套解释器。

## 职责

- `VmModule` / `Instruction` / `VmConfig` / `VmExit` / `Interpreter`
- 稠密 `SlotTable` / `SlotValue`（仅 typed 句柄）与 `Frame` / `FrameStack`
- `VmConstant` 常量表与封闭指令：`LoadConstant` / `Move` / `Guard` / `Reject` / `Safepoint` / `Return`
- 协作式 `CancellationToken` 与步数预算（在 safepoint / 步进处检查）
- `ExecutionLease`：执行期 object / numeric root 登记，Drop 注销
- 终态：Reference 解释循环归属本 crate；语义 / provider 经 host 回调由 engine 提供

## 非职责

不解析方言、不做 Mathematica 名称分派、不拥有 M-Graph admission、不拥有持久数学 payload、不自建 GC heap。  
不算第二个 `athena-engine`，也不是通用栈式 VM。

## 依赖

```text
athena-types → athena-gc → … → athena-rewriter → athena-vm → athena-engine → athena
```

## 验证

```sh
cargo test -p athena-vm --test main
cargo doc -p athena-vm --no-deps
```
