# athena-table

Athena 的类型化结构数据、列式存储与惰性关系查询基础库。

- 借鉴 Polars 的表达式 / 计划分离与 Arrow 的列式交换边界。
- 产品真相源是 `Table` / `LazyTable`，不是第三方 DataFrame API。
- **不包含**机器学习 estimator；有状态 ML 归 DXO/Titan。
- 固定宽度列与分块 I/O 复用 `athena-ndarray` 的 storage / memory budget。

已实现：`schema` · `column` · `LazyTable` · `LogicalPlan` 合同骨架。
