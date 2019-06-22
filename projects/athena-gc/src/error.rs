//! `athena-gc` 本地错误（后续可映射到 `athena-types` 诊断码）。

use core::fmt;

/// GC / arena 失败原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GcError {
    /// 超过 `max_arena_bytes`。
    ArenaBytesLimit {
        /// 请求后总量。
        requested_total: usize,
        /// 上限。
        limit: usize,
    },
    /// 超过 `max_segment_count`。
    SegmentCountLimit {
        /// 当前段数。
        count: usize,
        /// 上限。
        limit: usize,
    },
    /// 超过 `max_limbs`（按 u64 limb 计）。
    LimbLimit {
        /// 请求 limb 数。
        requested: usize,
        /// 上限。
        limit: usize,
    },
    /// 超过 `max_scratch_bytes`。
    ScratchBytesLimit {
        /// 请求后总量。
        requested_total: usize,
        /// 上限。
        limit: usize,
    },
    /// Scratch bump 不足（未先 ensure / 容量不够）。
    ScratchUnderrun {
        /// 需要的额外字节。
        need: usize,
        /// 剩余字节。
        remaining: usize,
    },
    /// 容量为零或非法。
    InvalidCapacity,
    /// 对象 / generation 已失效。
    StaleObject {
        /// 索引。
        index: u32,
        /// 期望 generation。
        expected_generation: u32,
    },
    /// 指针不属于本 heap 的已知 allocation。
    UnknownAllocation,
    /// Heap 正被借用，无法经 registry 重入。
    HeapBusy,
    /// 线程本地 registry 已销毁（线程退出路径）。
    RegistryUnavailable,
    /// Rust-owned 与 GC-owned 生命周期混用（拒绝 free，避免 double-free）。
    LifecycleMismatch,
    /// Graph domain block 不属于当前 heap。
    WrongHeap,
}

impl fmt::Display for GcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArenaBytesLimit { requested_total, limit } => {
                write!(f, "arena bytes limit: requested_total={requested_total} limit={limit}")
            }
            Self::SegmentCountLimit { count, limit } => {
                write!(f, "segment count limit: count={count} limit={limit}")
            }
            Self::LimbLimit { requested, limit } => {
                write!(f, "limb limit: requested={requested} limit={limit}")
            }
            Self::ScratchBytesLimit { requested_total, limit } => {
                write!(f, "scratch bytes limit: requested_total={requested_total} limit={limit}")
            }
            Self::ScratchUnderrun { need, remaining } => {
                write!(f, "scratch underrun: need={need} remaining={remaining}")
            }
            Self::InvalidCapacity => write!(f, "invalid capacity"),
            Self::StaleObject { index, expected_generation } => {
                write!(f, "stale object: index={index} generation={expected_generation}")
            }
            Self::UnknownAllocation => write!(f, "unknown allocation"),
            Self::HeapBusy => write!(f, "heap busy"),
            Self::RegistryUnavailable => write!(f, "gc registry unavailable"),
            Self::LifecycleMismatch => write!(f, "numeric reclaim authority mismatch"),
            Self::WrongHeap => write!(f, "graph domain block belongs to a different heap"),
        }
    }
}

impl std::error::Error for GcError {}

/// `athena-gc` 结果别名。
pub type Result<T> = core::result::Result<T, GcError>;
