//! 可观测统计（benchmark 报告字段）。

/// Heap / GC 累计统计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HeapStats {
    /// 累计 arena 分配次数。
    pub allocation_count: u64,
    /// 累计分配字节（含 header）。
    pub total_arena_bytes_allocated: usize,
    /// 峰值 arena resident（各 segment capacity 之和的峰值）。
    pub peak_arena_bytes: usize,
    /// 峰值 scratch used。
    pub peak_scratch_bytes: usize,
    /// 曾分配的 segment 数（含已 reclaim）。
    pub segments_allocated: u64,
    /// 已 reclaim 的 segment 数。
    pub segments_reclaimed: u64,
    /// `collect` 累计调用次数。
    pub collect_count: u64,
    /// 累计 `collect` 耗时（纳秒，粗计）。
    pub gc_time_ns: u64,
    /// `Drop` 遇 `HeapBusy` 而泄漏的次数。
    pub drop_busy_leaks: u64,
    /// ExplicitRelease 路径作用在 TracingSweep block（或反向）的次数。
    pub lifecycle_mismatch: u64,
}
