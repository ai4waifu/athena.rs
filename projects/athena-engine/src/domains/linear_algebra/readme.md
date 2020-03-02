# 线性代数

## 矩阵表示与算法分流

```mermaid
flowchart LR
    Ref[MatrixRef] --> Meta["MatrixParent + Shape + Layout"]
    Meta --> Domain{element domain}
    Domain -->|exact rational| Exact["Bareiss / RREF"]
    Domain -->|machine real| LU["partial-pivot LU"]
    Exact --> EResult["ExactSolveResult\nexact rank / determinant"]
    LU --> MResult["MachineSolveResult\npivot + residual + witness"]
    EResult --> Result[LinearAlgebraResult]
    MResult --> Result
```

| 层 | 类型 | 对算法的影响 |
|---|---|---|
| 身份 | `MatrixRef`、`MatrixParent` | 元素域和对象归属 |
| 维度 | `MatrixShape` | matmul、solve、slice 的合法性 |
| 布局 | `Layout`、`StorageOrder` | 索引和后端访问方式 |
| 精确路径 | `ExactRrefResult`、`ExactSolveResult` | exact guarantee、解空间 |
| 机器路径 | `MachineLuFactorization`、`MachineSolveWitness` | pivot threshold、误差与残差 |

## 处理流程

```mermaid
sequenceDiagram
    participant Q as LinearAlgebraRequest
    participant O as MatrixObjectStore
    participant P as shape / parent checks
    participant A as exact or machine kernel
    Q->>O: resolve MatrixRef
    O->>P: validate element domain and dimensions
    P->>A: select algorithm
    A-->>Q: value + SolveDisposition + AlgorithmGuarantee
```

`det_bareiss`、`rank_exact`、`rref_rational` 和 `solve_exact` 不经机器浮点。`lu_partial_pivot`、`rank_machine` 和 `solve_machine` 必须暴露 pivot threshold 与 witness。欠定系统返回解空间信息，一个向量不能冒充完整解集。

| 操作 | 预检查 | 结果 |
|---|---|---|
| `matmul` | inner dimension、parent | 新矩阵 |
| `slice_matrix` | `IndexSpec`、一基边界 | 保留布局语义的切片 |
| exact solve | 精确域、shape | 唯一解、仿射族或不相容 |
| machine solve | pivot 与容差 | 近似解、残差、witness |

源码从 [parent.rs](./parent.rs) / [shape.rs](./shape.rs) / [value.rs](./value.rs) 开始，随后读 [ops.rs](./ops.rs) / [index.rs](./index.rs)，最后对照 [exact.rs](./exact.rs)、[machine.rs](./machine.rs)、[status.rs](./status.rs) 与 [result.rs](./result.rs)。

`linear_algebra` 负责矩阵身份、shape/layout、精确与机器数值路径，以及线性方程组的结果合同。

## 能力

- `MatrixValue`、`MatrixShape`、`Layout`、`MatrixRef` 和 parent
- `matmul`、`transpose`、`hadamard`、切片与一基索引
- 精确有理数路径：Bareiss 行列式、秩、RREF 和线性求解
- 机器路径：部分主元 LU、秩和求解
- `LinearAlgebraRequest`、`LinearAlgebraResult` 与 `LinearAlgebraValue`
- `AlgorithmGuarantee`、`SolveDisposition` 与 machine witness

精确路径和机器路径分别报告保证。机器浮点近似不能冒充 exact，也不能将一个特解冒充完整解空间。

## 边界

矩阵逻辑身份、domain、shape 和 view 与 `athena-ndarray` 的存储布局分开。数组 chunk、stride 和 out-of-core 生命周期由数组基础设施负责。方言 lowering 不在本模块。

## 测试

核心运算和结果合同位于 `projects/athena-engine/tests/domains/linear_algebra/`，矩阵对象与跨域 view 还需结合 `athena-ndarray` 测试检查。
## 请求与执行

`LinearAlgebraRequest` 在矩阵引用、shape、parent 和算法模式上保持显式。精确请求走 `rref_rational`、`rank_exact`、`det_bareiss` 或 `solve_exact`，机器请求走 `lu_partial_pivot`、`rank_machine` 或 `solve_machine`。结果通过 `LinearAlgebraResult` 区分精确证书、机器 witness、无解和资源状态。

## 文件地图

`parent.rs`/`shape.rs` 定义矩阵身份和布局，`object_ref.rs` 管理 session 引用，`exact.rs` 与 `machine.rs` 是两条算法路径，`ops.rs` 是矩阵操作，`index.rs` 负责一基索引和切片，`status.rs` 定义保证，`result.rs` 负责领域返回。

## 语义约束

矩阵运算前检查 parent、shape 和布局兼容性。机器 LU 的 pivot threshold、残差和 witness 必须可读取。pivot 失败或预算截断只能返回相应状态，不能降级为 exact rank 或完整 nullspace。
