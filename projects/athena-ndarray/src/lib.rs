//! CAS N 维数组与超内存分块存储基础。
//!
//! 提供 NumPy 式 shape / indexing 合同与 out-of-core storage，**不是** Titan Tensor 运行时。
//! 不定义 `Device` / Autograd / kernel dispatch；逐元素数值语义委托给元素类型所属层。

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod array;
mod error;
mod shape;
mod storage;

pub use array::{Array, ArrayView, ChunkedArray, array1d};
pub use error::ArrayError;
pub use shape::{Axis, ChunkPlan, LogicalShape, MemoryBudget};
pub use storage::{ArrayStorage, ChunkStore, InMemoryStorage, StorageCapabilities, StoreCapabilities};
