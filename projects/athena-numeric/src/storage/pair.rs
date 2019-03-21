//! 物理 `{meta, Magnitude}` 对：仅 Drop/Clone/union 访问。
//!
//! `meta` 各位的**语义**由外层 `Natural` / `Integer` / … 定义，本类型不充当跨类型语义包装。
#![allow(unsafe_code)]

use core::{
    hash::{Hash, Hasher},
    mem,
    ptr::NonNull,
};
use std::{cell::RefCell, rc::Rc};

use athena_gc::{GcHeap, heap_id_for_limbs};

use super::{
    meta::{
        META_SIGN_BIT, Mode, encode_heap_meta, encode_limb1_meta, encode_limb2_meta, encode_zero_meta, heap_len, is_negative,
        mode_of, try_mode_of,
    },
    owned::OwnedLimbBuffer,
    union::{HeapPayload, Magnitude},
    view::LimbView,
};

/// 物理布局：`meta` + `union Magnitude`（24 bytes on LP64）。
///
/// 所有 union 字段读写必须经本类型；外部禁止猜读 `magnitude`。
#[repr(C)]
pub(crate) struct MagnitudePair {
    meta: usize,
    magnitude: Magnitude,
}

impl MagnitudePair {
    /// 规范零。
    #[inline]
    pub(crate) fn zero() -> Self {
        Self { meta: encode_zero_meta(), magnitude: Magnitude { limb1: 0 } }
    }

    /// 由 `u64` 构造。
    #[inline]
    pub(crate) fn from_u64(n: u64) -> Self {
        if n == 0 { Self::zero() } else { Self { meta: encode_limb1_meta(false), magnitude: Magnitude { limb1: n } } }
    }

    /// 由双 limb 构造（`hi != 0`；否则退化为 `from_u64(lo)`）。
    #[inline]
    pub(crate) fn from_limb2(limbs: [u64; 2]) -> Self {
        if limbs[1] == 0 {
            Self::from_u64(limbs[0])
        }
        else {
            Self { meta: encode_limb2_meta(false), magnitude: Magnitude { limb2: limbs } }
        }
    }

    /// 由 `u128` 构造（结果为 Zero / Limb1 / Limb2，**不经堆**）。
    #[inline]
    pub(crate) fn from_u128(n: u128) -> Self {
        if n == 0 {
            Self::zero()
        }
        else if n <= u64::MAX as u128 {
            Self::from_u64(n as u64)
        }
        else {
            Self::from_limb2([n as u64, (n >> 64) as u64])
        }
    }

    /// 由 trim 后至多 2 个小端 limb 构造（Zero / Limb1 / Limb2，**不经堆**）。
    ///
    /// 调用方必须保证 `effective_len(limbs) ≤ 2`。更长幅度只能走 [`Self::from_limbs_in`]。
    #[inline]
    pub(crate) fn from_inline_limbs(limbs: &[u64]) -> Self {
        let el = effective_len(limbs);
        debug_assert!(el <= 2, "from_inline_limbs requires effective_len ≤ 2");
        match el {
            0 => Self::zero(),
            1 => Self::from_u64(limbs[0]),
            _ => Self::from_limb2([limbs[0], limbs[1]]),
        }
    }

    /// 由至多 2 个有效小端 limb 构造（executor 固定宽度 ≤ Limb2 结果）。
    #[inline]
    pub(crate) fn from_fixed_limbs(limbs: &[u64]) -> Self {
        Self::from_inline_limbs(limbs)
    }

    /// 由小端 limbs 构造，分配到指定 heap。
    ///
    /// trim 后 ≤ 2 limb 不分配；更长幅度是**唯一**可进 Heap 的正式构造路径。
    pub(crate) fn from_limbs_in(heap: &Rc<RefCell<GcHeap>>, limbs: &[u64]) -> athena_gc::Result<Self> {
        Self::from_limbs_in_with(heap, limbs, false)
    }

    /// 同 [`Self::from_limbs_in`]；`gc_owned` 时经 traced numeric 分配。
    pub(crate) fn from_limbs_in_with(
        heap: &Rc<RefCell<GcHeap>>,
        limbs: &[u64],
        gc_owned: bool,
    ) -> athena_gc::Result<Self> {
        let el = effective_len(limbs);
        match el {
            0 => Ok(Self::zero()),
            1 => {
                let limb = limbs[0];
                Ok(if limb == 0 {
                    Self::zero()
                }
                else {
                    Self { meta: encode_limb1_meta(false), magnitude: Magnitude { limb1: limb } }
                })
            }
            2 => {
                let lo = limbs[0];
                let hi = limbs[1];
                debug_assert!(hi != 0);
                Ok(Self { meta: encode_limb2_meta(false), magnitude: Magnitude { limb2: [lo, hi] } })
            }
            _ => {
                debug_assert!(limbs[el - 1] != 0);
                let buf = if gc_owned {
                    OwnedLimbBuffer::alloc_copy_gc_owned_in(heap, &limbs[..el], el)?
                }
                else {
                    OwnedLimbBuffer::alloc_copy_in(heap, &limbs[..el], el)?
                };
                Ok(Self::from_owned_heap(buf, el))
            }
        }
    }

    /// 接管已写好的 heap 缓冲（`len >= 3`，且 `len <= capacity`）。
    pub(crate) fn from_owned_heap(buf: OwnedLimbBuffer, len: usize) -> Self {
        debug_assert!(len >= 3);
        debug_assert!(len <= buf.capacity());
        debug_assert_ne!(buf.as_slice(len)[len - 1], 0);
        let payload = buf.into_payload();
        Self { meta: encode_heap_meta(len, false), magnitude: Magnitude { heap: payload } }
    }

    /// Heap limb 指针（仅 Heap mode）。
    pub(crate) fn heap_ptr(&self) -> Option<NonNull<u64>> {
        if matches!(self.mode(), Mode::Heap) {
            // SAFETY: Heap active。
            Some(unsafe { self.magnitude.heap.ptr })
        }
        else {
            None
        }
    }

    /// Heap 分配容量（limb 槽位数）；非 Heap 返回 `None`。
    #[inline]
    pub(crate) fn heap_capacity(&self) -> Option<usize> {
        if matches!(self.mode(), Mode::Heap) {
            // SAFETY: Heap active。
            Some(unsafe { self.magnitude.heap.capacity })
        }
        else {
            None
        }
    }

    /// Limb1 的数值；非 Limb1 返回 `None`。
    #[inline]
    pub(crate) fn as_limb1(&self) -> Option<u64> {
        if matches!(self.mode(), Mode::Limb1) {
            // SAFETY: Limb1 mode → limb1 active。
            Some(unsafe { self.magnitude.limb1 })
        }
        else {
            None
        }
    }

    /// Limb2 的数值；非 Limb2 返回 `None`。
    #[inline]
    pub(crate) fn as_limb2(&self) -> Option<[u64; 2]> {
        if matches!(self.mode(), Mode::Limb2) {
            // SAFETY: Limb2 mode → limb2 active。
            Some(unsafe { self.magnitude.limb2 })
        }
        else {
            None
        }
    }

    /// 当前 mode。
    #[inline]
    pub(crate) fn mode(&self) -> Mode {
        mode_of(self.meta)
    }

    /// 逻辑 limb 长度（零亦为 1：`[0]`）。
    #[inline]
    pub(crate) fn limb_len(&self) -> usize {
        match self.mode() {
            Mode::Limb1 => 1,
            Mode::Limb2 => 2,
            Mode::Heap => heap_len(self.meta),
        }
    }

    /// 是否为零：合法 `Limb1` 且 `limb1 == 0`。
    #[inline]
    pub(crate) fn is_zero(&self) -> bool {
        if !matches!(self.mode(), Mode::Limb1) {
            return false;
        }
        // SAFETY: Limb1 mode → limb1 active。
        unsafe { self.magnitude.limb1 == 0 }
    }

    /// `meta` 负号位是否置位（含 semantic zero；外层自行解释）。
    #[inline]
    pub(crate) fn sign_bit(&self) -> bool {
        is_negative(self.meta)
    }

    /// 设置 `meta` 负号位（零幅度亦可保留 `-0`）。
    #[inline]
    pub(crate) fn set_sign_bit(&mut self, negative: bool) {
        if negative {
            self.meta |= META_SIGN_BIT;
        }
        else {
            self.meta &= !META_SIGN_BIT;
        }
    }

    /// `meta` 负号位（semantic zero 时忽略，恒返回 false）。
    #[inline]
    pub(crate) fn is_negative(&self) -> bool {
        !self.is_zero() && is_negative(self.meta)
    }

    /// 清除 sign 位的克隆（供 `Natural` / 幅度运算）。
    ///
    /// Heap `RustOwned` 分配失败时 panic。有 context 的热路径用 [`Self::try_clone_clear_sign`]。
    #[inline]
    pub(crate) fn clone_clear_sign(&self) -> Self {
        self.try_clone_clear_sign().unwrap_or_else(|e| panic!("gc Clone must stay on owner heap: {e}"))
    }

    /// 可失败清除 sign 位的 owning 复制（Living `19`）。
    #[inline]
    pub(crate) fn try_clone_clear_sign(&self) -> athena_gc::Result<Self> {
        let mut out = self.try_clone()?;
        out.meta &= !META_SIGN_BIT;
        Ok(out)
    }

    /// 设置符号；零保持 `Limb1(0)`（sign 可保留 don't-care，此处归零仅为便利）。
    #[inline]
    pub(crate) fn with_negative(mut self, negative: bool) -> Self {
        if self.is_zero() {
            return Self::zero();
        }
        if negative {
            self.meta |= META_SIGN_BIT;
        }
        else {
            self.meta &= !META_SIGN_BIT;
        }
        self
    }

    /// 一次分派后的只读 limb 视图（零 → `[0]`）。
    ///
    /// Living `19`：经 [`decode_magnitude`]；损坏 / reserved meta 回退为 `[0]`，禁止越界 `from_raw_parts`。
    #[inline]
    pub(crate) fn as_limbs(&self) -> &[u64] {
        match self.try_as_limbs() {
            Ok(limbs) => limbs,
            Err(_) => {
                static ZERO: [u64; 1] = [0];
                &ZERO
            }
        }
    }

    /// Checked limb 视图（Living `19` `decode_magnitude`）。
    #[inline]
    pub(crate) fn try_as_limbs(&self) -> Result<&[u64], athena_types::Diagnostic> {
        Ok(super::decode_magnitude(self.meta, &self.magnitude)?.limbs())
    }

    /// 可失败 owning 复制。
    ///
    /// - Limb1 / Limb2：栈拷贝。
    /// - Heap `GcOwned`：再登记一条 [`athena_gc::NumericRoot`]（无 limb 分配）。
    /// - Heap `RustOwned`：同堆 `alloc_copy`（唯一允许的隐式分配 owning 复制入口；公共路径应优先 [`Self::try_clone`] + context）。
    pub(crate) fn try_clone(&self) -> athena_gc::Result<Self> {
        match try_mode_of(self.meta).map_err(|_| athena_gc::GcError::UnknownAllocation)? {
            Mode::Limb1 => {
                // SAFETY: Limb1 active。
                let limb = unsafe { self.magnitude.limb1 };
                Ok(Self { meta: self.meta, magnitude: Magnitude { limb1: limb } })
            }
            Mode::Limb2 => {
                // SAFETY: Limb2 active。
                let limbs = unsafe { self.magnitude.limb2 };
                Ok(Self { meta: self.meta, magnitude: Magnitude { limb2: limbs } })
            }
            Mode::Heap => {
                let len = heap_len(self.meta);
                // SAFETY: Heap active。
                let (ptr, capacity, heap_id) = unsafe {
                    let heap = self.magnitude.heap;
                    (heap.ptr, heap.capacity, heap_id_for_limbs(heap.ptr))
                };
                let n = len.min(capacity);
                match GcHeap::numeric_ownership_registered(heap_id, ptr) {
                    Ok(athena_gc::NumericOwnership::GcOwned) => {
                        // Living `19`：共享 Clone 再登记一条 NumericRoot，不拷贝 limb。
                        let _ = GcHeap::register_numeric_root_registered(heap_id, ptr, athena_gc::RootKind::Numeric)?;
                        Ok(Self {
                            meta: self.meta,
                            magnitude: Magnitude { heap: HeapPayload { ptr, capacity } },
                        })
                    }
                    _ => {
                        // SAFETY: n <= capacity。
                        let src = unsafe { core::slice::from_raw_parts(ptr.as_ptr(), n) };
                        let buf = OwnedLimbBuffer::alloc_copy_on(heap_id, src, capacity.max(n.max(1)))?;
                        let payload = buf.into_payload();
                        Ok(Self { meta: self.meta, magnitude: Magnitude { heap: payload } })
                    }
                }
            }
        }
    }

    /// Kernel 视图。
    #[inline]
    pub(crate) fn as_view(&self) -> LimbView<'_> {
        LimbView::from_slice(self.as_limbs())
    }

    /// Checked kernel 视图。
    #[inline]
    pub(crate) fn try_as_view(&self) -> Result<LimbView<'_>, athena_types::Diagnostic> {
        Ok(LimbView::from_slice(self.try_as_limbs()?))
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
            Mode::Limb1 => {
                // limb1 == 0 合法（semantic zero）；无额外 assert。
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

impl Default for MagnitudePair {
    fn default() -> Self {
        Self::zero()
    }
}

/// Owning clone of the physical pair.
///
/// Limb1 / Limb2 are infallible copies. Heap `GcOwned` shares via [`NumericRoot`]（无 limb 分配）。
/// Heap `RustOwned` allocates on the owner heap（Living `19`：应优先 `try_clone` / `try_clone_in`）。
///
/// # Panic
///
/// Heap `RustOwned` clone panics if allocation fails. Session `GcOwned` 路径不分配 limb，不因此 panic。
impl Clone for MagnitudePair {
    fn clone(&self) -> Self {
        self.try_clone().unwrap_or_else(|e| panic!("gc Clone must stay on owner heap: {e}"))
    }
}

impl Drop for MagnitudePair {
    fn drop(&mut self) {
        let Ok(Mode::Heap) = try_mode_of(self.meta)
        else {
            return;
        };
        // SAFETY: Heap mode → heap active。
        let payload = unsafe { self.magnitude.heap };
        self.meta = encode_zero_meta();
        self.magnitude = Magnitude { limb1: 0 };
        OwnedLimbBuffer::dealloc_heap(payload);
    }
}

impl PartialEq for MagnitudePair {
    fn eq(&self, other: &Self) -> bool {
        self.is_negative() == other.is_negative() && self.as_limbs() == other.as_limbs()
    }
}

impl Eq for MagnitudePair {}

impl Hash for MagnitudePair {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.is_negative().hash(state);
        self.as_limbs().hash(state);
    }
}

impl core::fmt::Debug for MagnitudePair {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MagnitudePair").field("mode", &self.mode()).field("limbs", &self.as_limbs()).finish()
    }
}

/// 用新值替换 `slot`（先写新 storage/meta，再释放旧 heap）。
pub(crate) fn replace_with(slot: &mut MagnitudePair, new: MagnitudePair) {
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
