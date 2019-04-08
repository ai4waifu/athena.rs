//! 外部 oracle / copy-boundary adapter。
//!
//! **不进**默认 `KernelTable` dispatch。仅用于差分测试、fuzz，或只能操作
//! 自有 bigint 对象的库（`Athena limbs → temporary foreign → copy back`）。
//!
//! 若外部函数接受 limb 指针且不接管 allocator，应作为可选
//! [`crate::kernel`] 条目，而不是本模块的 object 路径。

/// 独立 schoolbook mpn 参考（差分 / fuzz；不进生产 `KernelTable`）。
pub mod mpn_oracle;

#[cfg(feature = "native-accelerated")]
pub mod native;
