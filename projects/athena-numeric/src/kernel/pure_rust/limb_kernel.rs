//! 兼容门面：实现已迁至 [`crate::kernel::portable`]。
//!
//! 保留本路径以免 `pure_rust::limb_kernel` 调用方在 Living 17 迁移中断。

pub(crate) use crate::kernel::portable::*;
