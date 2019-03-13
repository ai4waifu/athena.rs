//! `meta` 位编码：mode / sign / heap_len。
//!
//! 各位是否**语义有效**由外层类型决定（见 Living 14）。
//! `Natural`：sign 位为 don't-care，读取时视为 NonNegative，不要求物理清零。
//! 零 = 合法 `Limb1` 且 `limb1 == 0`（无独立 Zero mode）。

/// mode 掩码（bit 0..1）。
pub(crate) const MODE_MASK: usize = 0b11;
/// Limb1（含 `limb1 == 0` 的 zero）。
pub(crate) const MODE_LIMB1: usize = 0b00;
/// Limb2。
pub(crate) const MODE_LIMB2: usize = 0b01;
/// Heap。
pub(crate) const MODE_HEAP: usize = 0b10;
/// 保留 / 非法（不得当作 zero）。
pub(crate) const MODE_RESERVED: usize = 0b11;
/// sign 位（bit 2）；`Natural` 不解释；`Integer` 等有符号外层仅对**非零**解释。
pub(crate) const META_SIGN_BIT: usize = 1 << 2;
/// heap len 起始位移（bit 3..）。
pub(crate) const LEN_SHIFT: usize = 3;

/// `Natural` Eq/Hash/fingerprint 相关位：mode + heap_len（忽略 sign）。
pub(crate) const NAT_RELEVANT_MASK: usize = !META_SIGN_BIT;

/// 宽度 / 表示 mode（无独立 Zero）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// 单 limb（`limb1 == 0` 即为 semantic zero）。
    Limb1,
    /// 双 limb。
    Limb2,
    /// 堆缓冲（len ≥ 3）。
    Heap,
}

/// 从 meta 取出 mode。
#[inline]
pub(crate) fn mode_of(meta: usize) -> Mode {
    try_mode_of(meta).unwrap_or_else(|_| panic!("reserved magnitude mode"))
}

/// 校验 mode 位（`MODE_RESERVED` 拒绝，不得当 zero）。
#[inline]
pub(crate) fn try_mode_of(meta: usize) -> Result<Mode, ()> {
    match meta & MODE_MASK {
        MODE_LIMB1 => Ok(Mode::Limb1),
        MODE_LIMB2 => Ok(Mode::Limb2),
        MODE_HEAP => Ok(Mode::Heap),
        MODE_RESERVED => Err(()),
        _ => unreachable!("mode bits only use 2 bits"),
    }
}

/// Limb1 meta（可选符号；零时 sign 仍可为 don't-care，不必清零）。
#[inline]
pub(crate) fn encode_limb1_meta(negative: bool) -> usize {
    MODE_LIMB1 | if negative { META_SIGN_BIT } else { 0 }
}

/// 语义零的默认 meta（`Limb1`，sign 位清零仅为构造便利，非 invariant）。
#[inline]
pub(crate) fn encode_zero_meta() -> usize {
    MODE_LIMB1
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

/// 是否负号位（调用方须先排除 semantic zero）。
#[inline]
pub(crate) fn is_negative(meta: usize) -> bool {
    meta & META_SIGN_BIT != 0
}
