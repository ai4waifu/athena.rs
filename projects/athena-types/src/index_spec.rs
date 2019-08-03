//! 中性索引规格（Living `27` · 禁止 `Part` / `Span` 进入核心）。

/// 整数下标（已由方言 lowering 规范化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerIndex(pub i64);

/// 相对末端的偏移。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegerOffset(pub i64);

/// 领域专用索引规格句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexSpecId(pub u32);

/// 单轴索引规格。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IndexSpec {
    /// 标量下标。
    Scalar(IntegerIndex),
    /// 闭区间步进范围。
    Range {
        /// 起点。
        start: IntegerIndex,
        /// 终点。
        end: IntegerIndex,
        /// 步长。
        step: i64,
    },
    /// 该轴全部元素。
    All,
    /// 相对末端的下标。
    EndRelative(IntegerOffset),
    /// 笛卡尔积式多规格（嵌套轴）。
    Cartesian(Vec<IndexSpec>),
    /// 领域注册的索引规格。
    DomainSpecific(IndexSpecId),
}
