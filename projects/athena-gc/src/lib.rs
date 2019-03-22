//! Athena CAS runtime GC heap。
//!
//! Arena 是 tracing GC 的底层组织方式，不是 numeric 私有临时池。
//! 本 crate 不依赖 `athena-numeric` / `athena-ir` / `athena-engine`。

#![deny(missing_docs)]

mod batch;
mod budget;
mod error;
mod header;
mod heap;
mod ids;
mod mode;
mod object;
mod registry;
mod root;
mod scratch;
mod segment;
mod stats;
mod trace;

pub use batch::{AllocationAccounting, NumericBatch};
pub use budget::HeapBudget;
pub use error::{GcError, Result};
pub use header::{AllocationHeader, BlockKind, MarkState, NumericOwnership};
pub use heap::{CollectReport, GcHeap, GraphDomainBlock, NumericBlock, NumericBumpMark};
pub use ids::{GcObjectId, HeapId, RootToken, SegmentId};
pub use mode::{GcController, GcDeferGuard, GcMode, GcPinGuard, GcPressure, GcSuspendGuard};
pub use object::ObjectBlock;
pub use root::{GcRoot, NumericRoot, RootKind, RootRegistry};
pub use scratch::{ScratchArena, ScratchMark};
pub use segment::{SegmentKind, SegmentMeta};
pub use stats::HeapStats;
pub use trace::{EmptyObjectGraph, ObjectGraph, Trace, Tracer};

/// 与 Living 文档一致的别名。
pub type ArenaHeap = GcHeap;

pub use heap::heap_id_for_limbs;
pub use registry::record_drop_busy_leak;

/// 经 registry 借用已登记 heap（闭包可失败；供 numeric Clone / 分配）。
pub fn with_registered_heap<R>(id: HeapId, f: impl FnOnce(&mut GcHeap) -> Result<R>) -> Result<R> {
    registry::with_heap(id, f).and_then(core::convert::identity)
}
