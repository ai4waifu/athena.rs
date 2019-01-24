//! `TaggedMagnitude`：`meta` + 纯 `union Magnitude` 的安全封装。
#![allow(unsafe_code)]

use core::{
    hash::{Hash, Hasher},
    mem,
};

use super::{
    meta::{
        META_SIGN_BIT, Mode, encode_heap_meta, encode_limb1_meta, encode_limb2_meta, encode_zero_meta,
        heap_len, is_negative, mode_of,
    },
    owned::OwnedLimbBuffer,
    union::Magnitude,
    view::LimbView,
};

/// 规范零的静态 limb 视图（`as_limbs` 兼容 `[0]`）。
static ZERO_LIMB: [u64; 1] = [0];

/// 带 tag 的完整 magnitude（24 bytes on LP64）。
///
/// 所有 union 字段读写必须经本类型；外部禁止猜读 `magnitude`。
#[repr(C)]
pub(crate) struct TaggedMagnitude {
    meta: usize,
    magnitude: Magnitude,
}

impl TaggedMagnitude {
    /// 规范零。
    #[inline]
    pub(crate) fn zero() -> Self {
        Self { meta: encode_zero_meta(), magnitude: Magnitude { limb1: 0 } }
    }

    /// 由 `u64` 构造。
    #[inline]
    pub(crate) fn from_u64(n: u64) -> Self {
        if n == 0 {
            Self::zero()
        } else {
            Self { meta: encode_limb1_meta(false), magnitude: Magnitude { limb1: n } }
        }
    }

    /// 由双 limb 构造（`hi != 0`；否则退化为 `from_u64(lo)`）。
    #[inline]
    pub(crate) fn from_limb2(limbs: [u64; 2]) -> Self {
        if limbs[1] == 0 {
            Self::from_u64(limbs[0])
        } else {
            Self { meta: encode_limb2_meta(false), magnitude: Magnitude { limb2: limbs } }
        }
    }

    /// 由 `u128` 构造（结果为 Zero / Limb1 / Limb2，**不经堆**）。
    #[inline]
    pub(crate) fn from_u128(n: u128) -> Self {
        if n == 0 {
            Self::zero()
        } else if n <= u64::MAX as u128 {
            Self::from_u64(n as u64)
        } else {
            Self::from_limb2([n as u64, (n >> 64) as u64])
        }
    }

    /// 由至多 4 个小端 limb 构造（trim 后选 mode；3–4 limb 才进 Heap）。
    #[inline]
    pub(crate) fn from_fixed_limbs(limbs: &[u64]) -> Self {
        debug_assert!(limbs.len() <= 4);
        Self::from_limbs(limbs)
    }

    /// 由小端 limbs 构造（trim 后选 mode）。
    pub(crate) fn from_limbs(limbs: &[u64]) -> Self {
        let el = effective_len(limbs);
        match el {
            0 => Self::zero(),
            1 => {
                let limb = limbs[0];
                if limb == 0 {
                    Self::zero()
                } else {
                    Self { meta: encode_limb1_meta(false), magnitude: Magnitude { limb1: limb } }
                }
            }
            2 => {
                let lo = limbs[0];
                let hi = limbs[1];
                debug_assert!(hi != 0);
                Self { meta: encode_limb2_meta(false), magnitude: Magnitude { limb2: [lo, hi] } }
            }
            _ => {
                debug_assert!(limbs[el - 1] != 0);
                let buf = OwnedLimbBuffer::alloc_copy(&limbs[..el], el);
                let payload = buf.into_payload();
                Self { meta: encode_heap_meta(el, false), magnitude: Magnitude { heap: payload } }
            }
        }
    }

    /// Limb1 的数值；非 Limb1 返回 `None`。
    #[inline]
    pub(crate) fn as_limb1(&self) -> Option<u64> {
        if matches!(self.mode(), Mode::Limb1) {
            // SAFETY: Limb1 mode → limb1 active。
            Some(unsafe { self.magnitude.limb1 })
        } else {
            None
        }
    }

    /// Limb2 的数值；非 Limb2 返回 `None`。
    #[inline]
    pub(crate) fn as_limb2(&self) -> Option<[u64; 2]> {
        if matches!(self.mode(), Mode::Limb2) {
            // SAFETY: Limb2 mode → limb2 active。
            Some(unsafe { self.magnitude.limb2 })
        } else {
            None
        }
    }

    /// 当前 mode。
    #[inline]
    pub(crate) fn mode(&self) -> Mode {
        mode_of(self.meta)
    }

    /// 逻辑 limb 长度（Zero → 0）。
    #[inline]
    pub(crate) fn limb_len(&self) -> usize {
        match self.mode() {
            Mode::Zero => 0,
            Mode::Limb1 => 1,
            Mode::Limb2 => 2,
            Mode::Heap => heap_len(self.meta),
        }
    }

    /// 是否为零。
    #[inline]
    pub(crate) fn is_zero(&self) -> bool {
        matches!(self.mode(), Mode::Zero)
    }

    /// `meta` 负号位（Zero 时必须为 false）。
    #[inline]
    pub(crate) fn is_negative(&self) -> bool {
        !self.is_zero() && is_negative(self.meta)
    }

    /// 清除 sign 位的克隆（供 `Natural` / 幅度运算）。
    #[inline]
    pub(crate) fn clone_unsigned(&self) -> Self {
        let mut out = self.clone();
        out.meta &= !META_SIGN_BIT;
        out
    }

    /// 设置符号；零恒为 unsigned Zero。
    #[inline]
    pub(crate) fn with_negative(mut self, negative: bool) -> Self {
        if self.is_zero() {
            return Self::zero();
        }
        if negative {
            self.meta |= META_SIGN_BIT;
        } else {
            self.meta &= !META_SIGN_BIT;
        }
        self
    }

    /// 一次分派后的只读 limb 视图（Zero → `[0]`）。
    #[inline]
    pub(crate) fn as_limbs(&self) -> &[u64] {
        match self.mode() {
            Mode::Zero => &ZERO_LIMB,
            Mode::Limb1 => {
                // SAFETY: Limb1 mode → limb1 为 active field。
                unsafe { core::slice::from_ref(&self.magnitude.limb1) }
            }
            Mode::Limb2 => {
                // SAFETY: Limb2 mode → limb2 为 active field。
                unsafe { &self.magnitude.limb2 }
            }
            Mode::Heap => {
                let len = heap_len(self.meta);
                // SAFETY: Heap mode → heap 为 active；前 len 个 limb 已初始化。
                unsafe {
                    let heap = self.magnitude.heap;
                    debug_assert!(len <= heap.capacity);
                    core::slice::from_raw_parts(heap.ptr.as_ptr(), len)
                }
            }
        }
    }

    /// Kernel 视图。
    #[inline]
    pub(crate) fn as_view(&self) -> LimbView<'_> {
        LimbView::from_slice(self.as_limbs())
    }

    /// 拆成扁平字段（调用方接管 Drop 责任）。
    #[inline]
    pub(crate) fn into_parts(self) -> (usize, Magnitude) {
        let meta = self.meta;
        let magnitude = self.magnitude;
        mem::forget(self);
        (meta, magnitude)
    }

    /// 由已配对 parts 组装（`meta` 必须与 `magnitude` 一致）。
    #[inline]
    pub(crate) fn from_parts(meta: usize, magnitude: Magnitude) -> Self {
        let tagged = Self { meta, magnitude };
        #[cfg(debug_assertions)]
        tagged.debug_assert_invariants();
        tagged
    }

    /// Steal heap buffer；非 Heap 返回 `None`。Heap 时 self 变为 Zero。
    pub(crate) fn steal_heap(&mut self) -> Option<OwnedLimbBuffer> {
        if !matches!(self.mode(), Mode::Heap) {
            return None;
        }
        // SAFETY: Heap mode → heap active。
        let payload = unsafe { self.magnitude.heap };
        self.meta = encode_zero_meta();
        self.magnitude = Magnitude { limb1: 0 };
        Some(OwnedLimbBuffer::from_payload(payload))
    }

    #[cfg(debug_assertions)]
    fn debug_assert_invariants(&self) {
        match self.mode() {
            Mode::Zero => {}
            Mode::Limb1 => {
                // SAFETY: mode Limb1。
                let limb = unsafe { self.magnitude.limb1 };
                debug_assert_ne!(limb, 0, "Limb1 must be non-zero; use Zero mode");
            }
            Mode::Limb2 => {
                // SAFETY: mode Limb2。
                let limbs = unsafe { self.magnitude.limb2 };
                debug_assert_ne!(limbs[1], 0, "Limb2 high limb must be non-zero");
            }
            Mode::Heap => {
                let len = heap_len(self.meta);
                debug_assert!(len >= 3);
                // SAFETY: mode Heap。
                let heap = unsafe { self.magnitude.heap };
                debug_assert!(heap.capacity >= len);
                let top = unsafe { *heap.ptr.as_ptr().add(len - 1) };
                debug_assert_ne!(top, 0, "Heap must not have trailing zero");
            }
        }
    }
}

fn effective_len(limbs: &[u64]) -> usize {
    let mut n = limbs.len();
    while n > 0 && limbs[n - 1] == 0 {
        n -= 1;
    }
    n
}

impl Default for TaggedMagnitude {
    fn default() -> Self {
        Self::zero()
    }
}

impl Clone for TaggedMagnitude {
    fn clone(&self) -> Self {
        match self.mode() {
            Mode::Zero => Self::zero(),
            Mode::Limb1 => {
                // SAFETY: Limb1 active。
                let limb = unsafe { self.magnitude.limb1 };
                Self { meta: self.meta, magnitude: Magnitude { limb1: limb } }
            }
            Mode::Limb2 => {
                // SAFETY: Limb2 active。
                let limbs = unsafe { self.magnitude.limb2 };
                Self { meta: self.meta, magnitude: Magnitude { limb2: limbs } }
            }
            Mode::Heap => {
                let len = heap_len(self.meta);
                // SAFETY: Heap active。
                let (src, capacity) = unsafe {
                    let heap = self.magnitude.heap;
                    (core::slice::from_raw_parts(heap.ptr.as_ptr(), len), heap.capacity)
                };
                let buf = OwnedLimbBuffer::alloc_copy(src, capacity.max(len));
                let payload = buf.into_payload();
                Self { meta: self.meta, magnitude: Magnitude { heap: payload } }
            }
        }
    }
}

impl Drop for TaggedMagnitude {
    fn drop(&mut self) {
        if matches!(self.mode(), Mode::Heap) {
            // SAFETY: Heap mode → heap active。
            let payload = unsafe { self.magnitude.heap };
            self.meta = encode_zero_meta();
            self.magnitude = Magnitude { limb1: 0 };
            OwnedLimbBuffer::dealloc_heap(payload);
        }
    }
}

impl PartialEq for TaggedMagnitude {
    fn eq(&self, other: &Self) -> bool {
        self.is_negative() == other.is_negative() && self.as_limbs() == other.as_limbs()
    }
}

impl Eq for TaggedMagnitude {}

impl Hash for TaggedMagnitude {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.is_negative().hash(state);
        self.as_limbs().hash(state);
    }
}

impl core::fmt::Debug for TaggedMagnitude {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TaggedMagnitude")
            .field("mode", &self.mode())
            .field("limbs", &self.as_limbs())
            .finish()
    }
}

/// 用新值替换 `slot`（先写新 storage/meta，再释放旧 heap）。
pub(crate) fn replace_with(slot: &mut TaggedMagnitude, new: TaggedMagnitude) {
    let old_meta = slot.meta;
    let old_mag = slot.magnitude;
    slot.meta = new.meta;
    slot.magnitude = new.magnitude;
    mem::forget(new);
    if mode_of(old_meta) == Mode::Heap {
        // SAFETY: 旧 mode 为 Heap。
        let payload = unsafe { old_mag.heap };
        OwnedLimbBuffer::dealloc_heap(payload);
    }
}
