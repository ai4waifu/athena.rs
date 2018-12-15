//! 类型化列式表与惰性查询合同。
//!
//! 借鉴 Polars 查询架构与 Arrow 交换边界，**不是** pandas/Polars 复刻，也不承载 ML estimator。
//! 固定宽度列与分块计算复用 [`athena_ndarray`] 的 storage / memory budget。

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod column;
mod error;
mod plan;
mod schema;

pub use column::{ChunkedColumn, Column, RecordBatch, Table, column_from_store};
pub use error::TableError;
pub use plan::{LazyTable, LogicalPlan, TableExpr};
pub use schema::{Absence, Field, LogicalType, Schema};

/// Compatibility alias —— **不是**架构真相源。
pub type DataFrame = Table;
