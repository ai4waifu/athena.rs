//! Athena CAS runtime GC heap。
//!
//! Arena 是 tracing GC 的底层组织方式，不是 numeric 私有临时池。
//! 本 crate 不依赖 `athena-numeric` / `athena-ir` / `athena-engine`。

#![deny(missing_docs)]

mod budget;
mod error;
mod header;
mod heap;
mod ids;
mod mode;
mod root;
mod scratch;
mod segment;
mod stats;
mod trace;

pub use budget::HeapBudget;
pub use error::{GcError, Result};
pub use header::{AllocationHeader, BlockKind, MarkState};
pub use heap::{CollectReport, GcHeap, NumericBlock};
pub use ids::{GcObjectId, RootToken, SegmentId};
pub use mode::{GcController, GcDeferGuard, GcMode, GcPinGuard, GcPressure, GcSuspendGuard};
pub use root::{GcRoot, RootKind, RootRegistry};
pub use scratch::{ScratchArena, ScratchMark};
pub use segment::{SegmentKind, SegmentMeta};
pub use stats::HeapStats;
pub use trace::{Trace, Tracer};

/// 与 Living 文档一致的别名。
pub type ArenaHeap = GcHeap;
