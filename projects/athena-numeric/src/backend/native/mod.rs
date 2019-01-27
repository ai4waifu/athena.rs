//! 可选 native limb kernel 加速（feature `native-accelerated`）。
//!
//! 合同：只操作 Athena 提供的 `&[u64]` / `&mut [u64]`（mpn 风格），
//! 不得把外部 bigint 对象当作 Athena 值表示；不得在 backend 内 allocator/GC。
//! 若某库只能走 object API，必须经 copy 边界并单独记账，不得与零拷贝路径混报。

#![allow(dead_code)]

/// Native limb kernel 占位（启用 feature 后落地 `*_into` 实现）。
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAcceleratedBackend;
