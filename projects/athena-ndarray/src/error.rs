//! 数组 / 存储结构化错误（crate 本地；经 engine 编排时可升格为 `Diagnostic`）。

/// 数组错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayError {
    /// Shape 元素个数溢出。
    ShapeOverflow,
    /// 零内存预算。
    ZeroBudget,
    /// 单个元素已超过预算。
    BudgetTooSmall {
        /// 元素字节数。
        element_size: usize,
    },
    /// Storage 长度与 shape 不一致。
    LengthMismatch {
        /// 期望元素数。
        expected: u64,
        /// 实际元素数。
        actual: u64,
    },
    /// 请求工作集超过预算。
    BudgetExceeded {
        /// 请求元素数。
        requested: usize,
        /// 允许最大值。
        max: usize,
    },
    /// 区间算术溢出。
    RangeOverflow,
    /// 越界。
    OutOfBounds,
    /// 底层 storage 失败（细节不进入公共枚举）。
    Store,
}
