//! 可选 native limb 加速占位（feature `native-accelerated`）。
//!
//! 合同：优先 mpn 风格（Athena 提供 `&[u64]` / `&mut [u64]`）。
//! 若某库只能走 object API，必须留在 `foreign/` copy 边界并单独记账，
//! 不得与零拷贝 `kernel` 路径混报，也不得作为 `Integer`/`Natural` 存储。

#![allow(dead_code)]

/// Native limb / copy-boundary 占位（启用 feature 后落地）。
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAcceleratedAdapter;
