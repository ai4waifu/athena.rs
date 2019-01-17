//! 可选 native 加速 backend（未启用；结果必须写回 Athena canonical limbs）。

#![allow(dead_code)]

/// Native backend 占位（feature `native-accelerated` 启用后落地）。
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAcceleratedBackend;
