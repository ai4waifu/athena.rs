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
    /// 请求工作集超过驻留预算（按元素计）。
    BudgetExceeded {
        /// 请求元素数。
        requested: usize,
        /// 允许最大值。
        max: usize,
    },
    /// 驻留字节轴超限。
    ResidentBudgetExceeded {
        /// 请求后总量。
        requested_total: usize,
        /// 上限。
        limit: usize,
    },
    /// Scratch 字节轴超限。
    ScratchBudgetExceeded {
        /// 请求后总量。
        requested_total: usize,
        /// 上限。
        limit: usize,
    },
    /// Spill 字节轴超限。
    SpillBudgetExceeded {
        /// 请求后总量。
        requested_total: usize,
        /// 上限。
        limit: usize,
    },
    /// 同时打开的 chunk 数超限。
    OpenChunksExceeded {
        /// 请求后打开数。
        requested: usize,
        /// 上限。
        limit: usize,
    },
    /// 禁止在超预算时获取全表连续视图。
    FullMaterializeForbidden {
        /// 逻辑元素数。
        elements: u64,
        /// 驻留预算字节。
        resident_limit: usize,
    },
    /// 区间算术溢出。
    RangeOverflow,
    /// 越界。
    OutOfBounds,
    /// 底层 storage 失败（细节不进入公共枚举）。
    Store,
    /// 广播维不兼容。
    BroadcastIncompatible,
    /// Layout / view 与 shape 不一致。
    LayoutMismatch,
    /// 视图相对源 revision 已过期。
    StaleView {
        /// 视图绑定的修订。
        expected: u64,
        /// 当前修订。
        actual: u64,
    },
    /// 下层 `athena-gc` 错误。
    Gc(athena_gc::GcError),
}

impl From<athena_gc::GcError> for ArrayError {
    fn from(value: athena_gc::GcError) -> Self {
        Self::Gc(value)
    }
}
