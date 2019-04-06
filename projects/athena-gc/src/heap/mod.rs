//! `GcHeap`：分段、不移动的 bump 堆 + 对象区 + 追踪回收。
//!
//! Living `23`：子系统按所有权边界拆到本目录各文件。门面仅做模块声明与再导出。
#![allow(unsafe_code)]

mod allocation;
mod batch_mode;
mod collection;
mod graph_domain;
mod numeric;
mod object;
mod segment_store;
mod shared;
mod state;

pub use collection::CollectReport;
pub use graph_domain::GraphDomainBlock;
pub use numeric::{NumericBlock, NumericBumpMark};
pub use shared::heap_id_for_limbs;
pub use state::GcHeap;
