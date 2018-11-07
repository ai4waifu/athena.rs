# athena-graph

`athena-graph` 提供普通离散图，不承载 M-Graph 的等价类、witness、closure 或 solver 语义。小图可用内存邻接表，大图用 `athena-ndarray` 的 chunk store 承载 CSR/CSC 和算法工作区。依赖方向固定为 `athena-graph → athena-ndarray`。
