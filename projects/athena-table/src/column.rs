//! 列与 eager 表。

use athena_ndarray::{ArrayStorage, ChunkedArray, LogicalShape, MemoryBudget};

use crate::{Field, LazyTable, LogicalPlan, Schema, TableError};

/// 单列元数据 + storage-backed 值缓冲。
#[derive(Debug)]
pub struct ChunkedColumn<T, S> {
    field: Field,
    values: ChunkedArray<T, S>,
}

/// 首轮 `Column` 合同别名。
pub type Column<T, S> = ChunkedColumn<T, S>;

impl<T, S: ArrayStorage<T>> ChunkedColumn<T, S> {
    /// 绑定字段与分块数组；行数须匹配。
    pub fn new(field: Field, values: ChunkedArray<T, S>) -> Result<Self, TableError> {
        if values.shape().rank() != 1 {
            return Err(TableError::NonVectorColumn);
        }
        Ok(Self { field, values })
    }

    /// 字段。
    pub const fn field(&self) -> &Field {
        &self.field
    }

    /// 行数。
    pub fn len(&self) -> u64 {
        self.values.shape().element_count()
    }

    /// 是否为空列。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 底层数组。
    pub const fn values(&self) -> &ChunkedArray<T, S> {
        &self.values
    }

    /// 按预算分块扫描。
    pub fn for_each_chunk(&self, visit: impl FnMut(u64, &[T])) -> Result<(), TableError> {
        self.values.for_each_chunk(visit).map_err(TableError::from)
    }
}

/// 同构 record batch：多列共享行数。
#[derive(Debug)]
pub struct RecordBatch {
    schema: Schema,
    row_count: u64,
}

impl RecordBatch {
    /// 仅元数据 batch（物理列由调用方按 schema 挂载）。
    pub fn meta(schema: Schema, row_count: u64) -> Self {
        Self { schema, row_count }
    }

    /// Schema。
    pub const fn schema(&self) -> &Schema {
        &self.schema
    }

    /// 行数。
    pub const fn row_count(&self) -> u64 {
        self.row_count
    }
}

/// Eager 表句柄（首轮：schema + 行计数合同）。
#[derive(Debug, Clone)]
pub struct Table {
    schema: Schema,
    row_count: u64,
}

impl Table {
    /// 创建空表。
    pub fn empty(schema: Schema) -> Self {
        Self { schema, row_count: 0 }
    }

    /// 声明行数的表骨架（物理列后续挂载）。
    pub fn with_rows(schema: Schema, row_count: u64) -> Self {
        Self { schema, row_count }
    }

    /// Schema。
    pub const fn schema(&self) -> &Schema {
        &self.schema
    }

    /// 行数。
    pub const fn row_count(&self) -> u64 {
        self.row_count
    }

    /// 转为惰性扫描入口。
    pub fn lazy(self) -> LazyTable {
        LazyTable { plan: LogicalPlan::Scan { schema: self.schema, estimated_rows: self.row_count } }
    }
}

/// 构造一维 storage-backed 列的便利函数。
pub fn column_from_store<T, S: ArrayStorage<T>>(
    field: Field,
    store: S,
    budget: MemoryBudget,
) -> Result<ChunkedColumn<T, S>, TableError> {
    let len = store.len();
    let shape = LogicalShape::new([len])?;
    let values = ChunkedArray::new(shape, store, budget)?;
    ChunkedColumn::new(field, values)
}
