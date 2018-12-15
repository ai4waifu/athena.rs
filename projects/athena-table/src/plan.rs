//! 惰性表达式与逻辑计划。

use crate::{LogicalType, Schema, Table, TableError};

/// 列表达式（非 `athena-ir::Term`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableExpr {
    /// 列引用。
    Column(String),
    /// 字面量标记（payload 由上层解释）。
    Literal(String),
    /// 类型转换。
    Cast {
        /// 输入。
        expr: Box<TableExpr>,
        /// 目标类型。
        to: LogicalType,
    },
    /// 别名。
    Alias {
        /// 输入。
        expr: Box<TableExpr>,
        /// 新名。
        name: String,
    },
}

/// 逻辑查询计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalPlan {
    /// 扫描源。
    Scan {
        /// Schema。
        schema: Schema,
        /// 估计行数。
        estimated_rows: u64,
    },
    /// 投影。
    Project {
        /// 输入。
        input: Box<LogicalPlan>,
        /// 表达式。
        exprs: Vec<TableExpr>,
    },
    /// 过滤。
    Filter {
        /// 输入。
        input: Box<LogicalPlan>,
        /// 谓词。
        predicate: TableExpr,
    },
    /// 限制。
    Limit {
        /// 输入。
        input: Box<LogicalPlan>,
        /// 行数上限。
        n: u64,
    },
}

impl LogicalPlan {
    /// 计划输出 schema（首轮：Scan 直接返回；Project/Filter/Limit 继承或待校验）。
    pub fn schema(&self) -> &Schema {
        match self {
            Self::Scan { schema, .. } => schema,
            Self::Project { input, .. } | Self::Filter { input, .. } | Self::Limit { input, .. } => input.schema(),
        }
    }
}

/// 惰性表。
#[derive(Debug, Clone)]
pub struct LazyTable {
    pub(crate) plan: LogicalPlan,
}

impl LazyTable {
    /// 当前逻辑计划。
    pub const fn plan(&self) -> &LogicalPlan {
        &self.plan
    }

    /// 投影。
    pub fn select(self, exprs: impl Into<Vec<TableExpr>>) -> Self {
        Self { plan: LogicalPlan::Project { input: Box::new(self.plan), exprs: exprs.into() } }
    }

    /// 过滤。
    pub fn filter(self, predicate: TableExpr) -> Self {
        Self { plan: LogicalPlan::Filter { input: Box::new(self.plan), predicate } }
    }

    /// 限制行数。
    pub fn limit(self, n: u64) -> Self {
        Self { plan: LogicalPlan::Limit { input: Box::new(self.plan), n } }
    }

    /// 物化为 eager 表元数据（首轮不执行物理算子）。
    pub fn collect_meta(self) -> Result<Table, TableError> {
        let schema = self.plan.schema().clone();
        let rows = match &self.plan {
            LogicalPlan::Scan { estimated_rows, .. } => *estimated_rows,
            LogicalPlan::Limit { n, input } => match input.as_ref() {
                LogicalPlan::Scan { estimated_rows, .. } => (*estimated_rows).min(*n),
                _ => *n,
            },
            _ => 0,
        };
        Ok(Table::with_rows(schema, rows))
    }
}
