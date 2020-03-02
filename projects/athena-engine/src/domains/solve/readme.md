# 方程求解

## 问题：什么才算“解完了”

求解并不等于找到一个代入后成立的值。Athena 必须区分完整解集、经过证明的子集、局部数值根、一个实例以及仍可继续的搜索。

```mermaid
flowchart LR
    Input["constraints + variables + domain"] --> Normalize["normalize\nEquation · Inequality · Predicate"]
    Normalize --> Problem["SolveProblem\ngoal · policy · limits"]
    Problem --> Reflect["Reflector\n选择领域能力"]
    Reflect --> Linear[linear algebra adapter]
    Reflect --> Poly[univariate polynomial adapter]
    Reflect --> Future[other providers]
    Linear --> Branches[SolutionBranch]
    Poly --> Branches
    Future --> Branches
    Branches --> Coverage[CoverageStatus]
    Coverage --> Set[SolutionSet]
```

| 目标 | 结果语义 | 不能冒充 |
|---|---|---|
| 精确解集 | 所有 branch + full coverage | 一个可行实例 |
| 数值根 | 局部收敛、误差和初值信息 | 全局解集 |
| `FindInstance` 类目标 | 一个满足约束的 binding | 完备枚举 |
| 参数解 | binding + conditions + free variables | 单个向量 |
| 资源截断 | certified subset + `ResumeToken` | `Unknown` 或完整失败 |

## 从约束到领域算法

```mermaid
sequenceDiagram
    participant T as TermStore
    participant N as normalize
    participant S as SolveProblem
    participant A as domain adapter
    participant V as residual verifier
    T->>N: relational applications
    N->>S: ConstraintSet + BoundSymbol
    S->>A: SolveGoal + domain + limits
    A-->>V: candidate bindings / branches
    V-->>S: ResidualCertificate
    S-->>T: SolutionSet + CoverageStatus
```

`normalize_relational_application` 将中性 IR 变成 `Equation`、`Inequality`、逻辑连接和量词。`BoundSymbol` 与 `BindingId` 防止自由变量和局部绑定混淆。线性 adapter 消费 `ExactSolveResult` 或 machine witness，多项式 adapter 消费带完整性信息的因式分解。

## 跨领域责任

| 领域 | 向 Solve 提供什么 | Solve 额外负责什么 |
|---|---|---|
| 线性代数 | 唯一解、无解、欠定 disposition | 转成 branch、free variable 和 coverage |
| 多项式 | 因子、根候选、分解完整性 | multiplicity、遗漏风险、solution branches |
| 数值算法 | 局部近似根与误差 | 标记 local/approximate，不提升为 exact |
| M-Graph | 已知关系与 proof ref | 复用事实并记录 residual certificate |

源码阅读：[constraint.rs](./constraint.rs) → [normalize.rs](./normalize.rs) → [problem.rs](./problem.rs) / [goal.rs](./goal.rs) → [adapters_linear.rs](./adapters_linear.rs) / [adapters_univariate.rs](./adapters_univariate.rs) → [solution.rs](./solution.rs) / [coverage.rs](./coverage.rs)。测试见 [Solve tests](../../../tests/domains/solve/)。

`solve` 定义跨领域求解的数学合同：约束、问题、目标、解集、覆盖度、条件和可恢复 frontier。调度协议位于 `reasoning::solver`，两者保持分离。

## 公开入口

- `Constraint`、`SolveProblem`、`SolveGoal` 与 `SolutionSet`
- `BindingMap`、`BoundSymbol`、`ResidualCertificate`
- `CoverageStatus`、`SolveDomain`、`ResumeToken`
- 线性系统和单变量多项式的 adapter
- `normalize_constraint_conjunction`、`require_goal` 与 `execute_linear_system_goal`

结果必须说明解集覆盖范围、残差、条件和完备性。找到一个实例不等于找到完整解集，局部数值根也不等于全局解。

## 执行路径

`SolveGoal` 由 engine 的 Reflector 选择领域 capability，再调用线性代数或多项式 adapter。`Frontier` 和 `ResumeToken` 保存资源截断后的继续入口，证书通过统一 verifier 与 M-Graph admission。

## 边界与测试

本模块不解析源语言，不定义 `SolverRequest`，也不创建 `athena-solver` crate。合同测试位于 `projects/athena-engine/tests/domains/solve/`。
