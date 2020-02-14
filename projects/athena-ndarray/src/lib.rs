//! CAS N 维数组与超内存分块存储基础。
//!
//! 提供 NumPy 式 shape / indexing 合同与超内存存储，**不是** Titan Tensor 运行时。
//! 不定义 `Device` / Autograd / kernel dispatch；逐元素数值语义委托给元素类型所属层。
//! 身份 / 预算 / 简版 GC 合同与 `athena-gc` 对齐（`ArrayId` ≠ shape ≠ budget）。
//!
//! `unsafe` 默认 [`allow`]：ndarray 是布局 / 拷贝 / 热路径库，与 `athena-numeric` kernel 同属可写 unsafe 层。
//! 仍须局部审阅；禁止用 `forbid`/`deny` 整 crate 堵死实现。

#![deny(missing_docs)]
#![allow(unsafe_code)]

mod array;
mod budget;
mod error;
mod layout;
mod lifecycle;
mod shape;
mod storage;

pub use array::{Array, Array2d, ArrayView, ChunkedArray, array1d, array2d, array2d_from_storage};
pub use budget::{BudgetLedger, ChunkGuard};
pub use error::ArrayError;
pub use layout::{ArrayLayout, ArrayOrder, ArrayViewSpec, BroadcastSpec, permute_axes};
pub use lifecycle::{
    ArrayChunkId, ArrayChunkRecord, ArrayId, ArrayPublication, ArrayRevision, ArrayRevisionId, ArrayRevisionRecord, ArraySnapshot,
    ArraySnapshotId, ArraySnapshotRecord, ArrayTraceIndex, PublishedArray, RecordingTracer, allocate_array_chunk_id,
    allocate_array_revision_id, allocate_array_snapshot_id, finish_array_on_heap, publish_array_snapshot,
};
pub use shape::{Axis, ChunkPlan, LogicalShape, MemoryBudget};
pub use storage::{ArrayStorage, InMemoryStorage, StorageCapabilities};
