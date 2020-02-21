# 群论

`group` 提供有限群和置换群的 typed 对象、元素运算与请求分派。元素携带所属群身份，跨群运算返回 `ATHENA_GROUP_MISMATCH` 类结构化诊断。

## 公开入口

- `Group`、`GroupDescriptor`、`GroupElement`、`Permutation`、`Subgroup`
- `GroupRequest`、`GroupResult` 与 `GroupDomainValue`
- `canonical_permutation`、`multiply_group_elements`、`inverse_group_element`
- `group_membership`、`apply_group_homomorphism` 与 `project_quotient_element`
- `execute_group` 及带 table 的执行变体

当前实现围绕置换 presentation、BSGS、子群、同态和商结构。抽象群的完整 Cayley 表和更广泛的群算法按结果状态逐步加入。

## 边界

共享 parent、映射和 fingerprint 来自 `algebra`。群请求不解析文本，不复制 solver 调度协议，也不把群元素降级成无类型整数。证书和关系准入由 engine reasoning 层统一处理。

## 测试

群与相关扩张合同位于 `projects/athena-engine/tests/domains/algebra/`。


## 架构图

```mermaid
flowchart LR
    Request["group request"] --> Object["typed object / reference"]
    Object --> Execute["domain execution"]
    Execute --> Result["value + status"]
    Result --> Verify["verifier / evidence"]
    Verify --> Publish["ComputationResult / M-Graph"]
```

## 合同表

| 阶段 | 输入 | 输出 | 必须保留 |
|---|---|---|---|
| 构造 | domain object、parent、scope | typed reference | identity、revision |
| 计划 | request、limits、capability | domain plan | algorithm、budget |
| 执行 | canonical representation | value、candidate 或 frontier | provenance、diagnostic |
| 验证 | value、certificate、dependencies | accepted claim 或 reject | replay evidence |
| 发布 | verified result | structured result | status、coverage、conditions |

## 源码阅读顺序

```mermaid
flowchart TD
    A["request.rs"] --> B["object / value"]
    B --> C["algorithm modules"]
    C --> D["result.rs"]
    D --> E["tests/domains/group"]
```

先读 `request.rs`，确认输入的身份和资源字段。再读对象/值模块，确认 payload、parent 和生命周期。随后读算法实现，最后读 `result.rs` 与测试，核对成功、失败和资源受限分支。

## 结果与证据

| 情况 | 结果状态 | 可以做什么 |
|---|---|---|
| 独立验证通过 | `Exact` 或 `Verified` | 按证书保证继续组合 |
| 依赖假设或分支 | `Conditional` | 携带条件继续查询 |
| 只得到候选 | `Candidate` | 等待 verifier，不得准入 |
| 算法被预算截断 | `Partial` / `ResourceLimited` | 保存 frontier 后恢复 |
| 输入或能力不满足 | `Invalid` / `Unknown` | 读取结构化诊断 |

证据不是日志字段。它必须能说明输入对象、算法前置条件、依赖关系和重放方式。缓存只能复用计算产物，不能代替验证和准入。

## 测试矩阵

| 测试层 | 必须证明 |
|---|---|
| 对象与规范化 | identity、parent、canonical form |
| 算法 | 正常值、边界值、域不匹配、除零或无解 |
| 结果 | payload、status、coverage、diagnostic |
| 资源 | budget、取消、frontier、resume |
| 证据 | replay、冲突、candidate 与 admission |

## 明确边界

本模块不解析源文本，不负责 UI、render、N-API 或平台对象。跨领域调用必须使用显式 capability、embedding 或 TypedView，并保留来源 fingerprint 与 revision。新增算法必须同步新增结果状态、失败路径和测试，不得只增加一个函数名。
