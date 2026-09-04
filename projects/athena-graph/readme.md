# athena-graph

`athena-graph` 提供普通离散图的身份、存储、视图与 L0 结构原语， **不**承载 M-Graph 语义，也 **不**发布图论数学 claim。

## 四层目录

```text
identity/     GraphId · GraphRevision · GraphSnapshot · GraphSemantics · MutableGraph / GraphBuilder / ImmutableGraph
storage/      CSR/CSC · property · capability · conversion
views/        reversed · induced · edge-filtered + ViewMapping · ViewNodeRef
primitives/   frontier · bfs_order · UnionFind · 分量扫描原语
```

公开稳定构造路径：`GraphBuilder` → `ImmutableGraph`。`MutableGraph` 仅构造期（经 `graph_mut`）。

分量 / SCC / 拓扑标签仅经 `athena_graph::primitives`，须由 `athena-engine::graph_theory` 包装证书。

## 已实现

- **身份**：`GraphId` · `GraphRevision` · `NodeRef`/`EdgeRef`（绑 revision）· `ViewNodeRef`/`SourceNodeRef` ·
  `GraphSnapshot`
- **表示**：内存邻接表 · `CsrGraph` · `CscGraph`
- **视图**：`ReversedGraphView` · `InducedSubgraphView` · `EdgeFilteredView`（过期 `StaleView`）
- **转换**：`graph_to_csr` · `edge_list_to_csr` · `csr_to_csc`
- **L0 原语**：确定性 BFS / frontier · `UnionFind` ·
  `primitives::{connected_components, strongly_connected_components, topological_sort}`
- **Capability**：`GraphAlgorithmRequirements` · `GraphCapabilities`

M-Graph（`athena-engine::mgraph`）即使复用本 crate 的 CSR/frontier，也不属于 `athena-graph`。

小图可用内存邻接表；大图用 `athena-ndarray` 的 chunk store。依赖：`athena-graph → athena-ndarray`。
