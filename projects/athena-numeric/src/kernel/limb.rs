//! Limb 执行合同入口（默认实现：[`crate::kernel::portable`]）。
//!
//! `value` / `arithmetic` 只应依赖本模块，不得直接 `use crate::kernel::portable::...`
//! 或 `pure_rust::...`（测试与 ISA 绑定除外）。Machine kernel 仅借用 limb/scratch；
//! foreign bigint wrapper 不得进入本路径。

pub(crate) use crate::kernel::portable::*;
