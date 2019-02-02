# athena-graph

`athena-graph` 提供普通离散图的数据结构与算法原语， **不**承载 M-Graph 语义。

## 已实现

- **结构**：`Graph` · `NodeId`/`EdgeId` · `GraphRevision`
- **表示**：内存邻接表 · `CsrGraph` · `CscGraph`（按需从 CSR 构建）
- **视图**：`GraphView` · `ReversedGraphView` · `InducedSubgraphView` · `EdgeFilteredView`
- **转换**：`graph_to_csr` · `edge_list_to_csr` · `csr_to_csc`
- **算法原语**：BFS · topo sort · `connected_components` · `strongly_connected_components` · `UnionFind`
- **Capability**：`GraphAlgorithmRequirements` · `GraphCapabilities`

M-Graph（`athena-engine::mgraph`）是 scoped relation index + admission，即使复用本 crate 的 CSR/frontier，也不属于
`athena-graph`。

小图可用内存邻接表；大图用 `athena-ndarray` 的 chunk store。依赖：`athena-graph → athena-ndarray`。
