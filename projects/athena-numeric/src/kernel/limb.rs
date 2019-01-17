//! Limb 执行合同入口（默认实现：[`crate::backend::pure_rust::limb_kernel`]）。
//!
//! `value` / `arithmetic` 只应依赖本模块，不得直接 `use crate::backend::...`。

pub(crate) use crate::backend::pure_rust::limb_kernel::*;
