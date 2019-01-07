//! Typed property / weight columns（非 `HashMap<String>` 默认模型）。

use crate::GraphError;

/// 属性单元格缺失语义（不得压成同一 sentinel）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyCell<T> {
    /// 有值。
    Present(T),
    /// 显式缺失。
    Missing,
    /// 未知（未观测）。
    Unknown,
}

impl<T> PropertyCell<T> {
    /// 若为 [`Present`](Self::Present) 则返回引用。
    pub const fn as_present(&self) -> Option<&T> {
        match self {
            Self::Present(v) => Some(v),
            Self::Missing | Self::Unknown => None,
        }
    }
}

/// 与节点数或边数对齐的 typed 属性列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyColumn<T> {
    values: Vec<PropertyCell<T>>,
}

impl<T> PropertyColumn<T> {
    /// 以给定长度与单元格构造；长度必须等于 `expected_len`。
    pub fn try_from_cells(expected_len: u64, values: Vec<PropertyCell<T>>) -> Result<Self, GraphError> {
        let actual = values.len() as u64;
        if actual != expected_len {
            return Err(GraphError::PropertyLengthMismatch { expected: expected_len, actual });
        }
        Ok(Self { values })
    }

    /// 全部为 [`Present`](PropertyCell::Present) 的稠密列。
    pub fn try_from_dense(expected_len: u64, values: Vec<T>) -> Result<Self, GraphError> {
        let actual = values.len() as u64;
        if actual != expected_len {
            return Err(GraphError::PropertyLengthMismatch { expected: expected_len, actual });
        }
        Ok(Self { values: values.into_iter().map(PropertyCell::Present).collect() })
    }

    /// 全 [`Missing`](PropertyCell::Missing) 列。
    pub fn missing(len: u64) -> Self {
        Self { values: (0..len).map(|_| PropertyCell::Missing).collect() }
    }

    /// 列长。
    pub fn len(&self) -> u64 {
        self.values.len() as u64
    }

    /// 是否为空列。
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// 按下标读取。
    pub fn get(&self, index: u64) -> Option<&PropertyCell<T>> {
        self.values.get(index as usize)
    }

    /// 单元格切片。
    pub fn cells(&self) -> &[PropertyCell<T>] {
        &self.values
    }
}

/// 权重存储域标签（结构层；数学问题合同仍在 `graph_theory`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeightDomainTag {
    /// 无权重（边存在即权 1 的语义由算法层解释）。
    Unweighted,
    /// 精确整数权。
    ExactInteger,
    /// 精确有理权。
    ExactRational,
    /// 机器浮点权（不可伪装 exact 最优证明）。
    MachineReal,
    /// 任意精度实数权（enclosure 另议）。
    ArbitraryReal,
    /// 区间权。
    Interval,
    /// 热带半环。
    Tropical,
    /// 符号权。
    Symbolic,
}

/// 绑定域标签的权重列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightColumn<T> {
    domain: WeightDomainTag,
    values: PropertyColumn<T>,
}

impl<T> WeightColumn<T> {
    /// 构造；列长须匹配 `expected_len`（通常为边数）。
    pub fn try_new(domain: WeightDomainTag, expected_len: u64, values: Vec<PropertyCell<T>>) -> Result<Self, GraphError> {
        Ok(Self { domain, values: PropertyColumn::try_from_cells(expected_len, values)? })
    }

    /// 稠密 Present 权重列。
    pub fn try_dense(domain: WeightDomainTag, expected_len: u64, values: Vec<T>) -> Result<Self, GraphError> {
        Ok(Self { domain, values: PropertyColumn::try_from_dense(expected_len, values)? })
    }

    /// 域标签。
    pub const fn domain(&self) -> WeightDomainTag {
        self.domain
    }

    /// 底层属性列。
    pub const fn column(&self) -> &PropertyColumn<T> {
        &self.values
    }

    /// 列长。
    pub fn len(&self) -> u64 {
        self.values.len()
    }
}

/// 节点/边属性表（列式；列名稳定字符串，值 typed）。
#[derive(Debug, Clone, Default)]
pub struct PropertyStore<N, E> {
    node_columns: Vec<(String, PropertyColumn<N>)>,
    edge_columns: Vec<(String, PropertyColumn<E>)>,
}

impl<N, E> PropertyStore<N, E> {
    /// 空表。
    pub fn new() -> Self {
        Self { node_columns: Vec::new(), edge_columns: Vec::new() }
    }

    /// 插入节点属性列；长度须等于 `node_count`。
    pub fn insert_node_column(
        &mut self,
        name: impl Into<String>,
        node_count: u64,
        column: PropertyColumn<N>,
    ) -> Result<(), GraphError> {
        if column.len() != node_count {
            return Err(GraphError::PropertyLengthMismatch { expected: node_count, actual: column.len() });
        }
        let name = name.into();
        if let Some((_, existing)) = self.node_columns.iter_mut().find(|(n, _)| *n == name) {
            *existing = column;
        }
        else {
            self.node_columns.push((name, column));
        }
        Ok(())
    }

    /// 插入边属性列；长度须等于 `edge_count`。
    pub fn insert_edge_column(
        &mut self,
        name: impl Into<String>,
        edge_count: u64,
        column: PropertyColumn<E>,
    ) -> Result<(), GraphError> {
        if column.len() != edge_count {
            return Err(GraphError::PropertyLengthMismatch { expected: edge_count, actual: column.len() });
        }
        let name = name.into();
        if let Some((_, existing)) = self.edge_columns.iter_mut().find(|(n, _)| *n == name) {
            *existing = column;
        }
        else {
            self.edge_columns.push((name, column));
        }
        Ok(())
    }

    /// 按名取节点列。
    pub fn node_column(&self, name: &str) -> Option<&PropertyColumn<N>> {
        self.node_columns.iter().find(|(n, _)| n == name).map(|(_, c)| c)
    }

    /// 按名取边列。
    pub fn edge_column(&self, name: &str) -> Option<&PropertyColumn<E>> {
        self.edge_columns.iter().find(|(n, _)| n == name).map(|(_, c)| c)
    }

    /// 节点列个数。
    pub fn node_column_count(&self) -> usize {
        self.node_columns.len()
    }

    /// 边列个数。
    pub fn edge_column_count(&self) -> usize {
        self.edge_columns.len()
    }
}
