//! `meta` 位编码：mode / sign / heap_len。

/// mode 掩码（bit 0..1）。
pub(crate) const MODE_MASK: usize = 0b11;
/// Zero。
pub(crate) const MODE_ZERO: usize = 0b00;
/// Limb1。
pub(crate) const MODE_LIMB1: usize = 0b01;
/// Limb2。
pub(crate) const MODE_LIMB2: usize = 0b10;
/// Heap。
pub(crate) const MODE_HEAP: usize = 0b11;
/// sign 位（bit 2）；Natural 恒为 0。
pub(crate) const META_SIGN_BIT: usize = 1 << 2;
/// heap len 起始位移（bit 3..）。
pub(crate) const LEN_SHIFT: usize = 3;

/// 宽度 / 表示 mode。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// 规范零。
    Zero,
    /// 单 limb。
    Limb1,
    /// 双 limb。
    Limb2,
    /// 堆缓冲（len ≥ 3）。
    Heap,
}

/// 从 meta 取出 mode。
#[inline]
pub(crate) fn mode_of(meta: usize) -> Mode {
    match meta & MODE_MASK {
        MODE_ZERO => Mode::Zero,
        MODE_LIMB1 => Mode::Limb1,
        MODE_LIMB2 => Mode::Limb2,
        MODE_HEAP => Mode::Heap,
        _ => unreachable!("mode bits only use 2 bits"),
    }
}

/// Zero meta（无符号）。
#[inline]
pub(crate) fn encode_zero_meta() -> usize {
    MODE_ZERO
}

/// Limb1 meta（可选符号）。
#[inline]
pub(crate) fn encode_limb1_meta(negative: bool) -> usize {
    MODE_LIMB1 | if negative { META_SIGN_BIT } else { 0 }
}

/// Limb2 meta（可选符号）。
#[inline]
pub(crate) fn encode_limb2_meta(negative: bool) -> usize {
    MODE_LIMB2 | if negative { META_SIGN_BIT } else { 0 }
}

/// Heap meta：`len >= 3`，可选符号。
#[inline]
pub(crate) fn encode_heap_meta(len: usize, negative: bool) -> usize {
    debug_assert!(len >= 3, "heap len must be >= 3");
    MODE_HEAP | if negative { META_SIGN_BIT } else { 0 } | (len << LEN_SHIFT)
}

/// 读取 heap len；非 Heap mode 时 debug panic。
#[inline]
pub(crate) fn heap_len(meta: usize) -> usize {
    debug_assert_eq!(meta & MODE_MASK, MODE_HEAP);
    meta >> LEN_SHIFT
}

/// 是否负号位。
#[inline]
pub(crate) fn is_negative(meta: usize) -> bool {
    meta & META_SIGN_BIT != 0
}
