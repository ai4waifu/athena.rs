//! [`Natural`] 发布、从 limb/pair 构造与内部存储访问。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::Natural;
use crate::{
    kernel::{LimbBuffer, limb as limb_kernel},
    policy::execution_budget::NumericContext,
    storage::{MagnitudePair, Mode, gc_alloc_error},
};

impl Natural {
    /// 借用小端 limb（生命周期绑在 `&self`；禁止跨 move 持有）。
    pub fn as_limbs(&self) -> &[u64] {
        self.inner.as_limbs()
    }

    /// 可失败 owning 复制（Heap 经 owner heap 分配；服从 `ctx` 入口预算）。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        Ok(Self::from_pair(self.inner.try_clone().map_err(gc_alloc_error)?))
    }

    /// 由小端 limb 构造（已规范化；经 [`NumericContext::portable_default`] 发布）。
    ///
    /// trim 后 ≤ 2 limb 不分配；更长幅度走 GC，失败返回 Diagnostic。
    /// 需要绑定 heap / 预算时请用 [`Self::from_limbs_in`]。
    pub fn from_limbs(limbs: Vec<u64>) -> Result<Self> {
        Self::from_limb_slice_in(&NumericContext::portable_default(), &limbs)
    }

    /// 由小端 limb 构造到 `ctx` 绑定的 heap。
    pub fn from_limbs_in(ctx: &NumericContext, limbs: Vec<u64>) -> Result<Self> {
        Self::from_limb_slice_in(ctx, &limbs)
    }

    /// 由 limb 切片发布到 `ctx` heap（无额外 `Vec`）。
    pub(crate) fn from_limb_slice_in(ctx: &NumericContext, limbs: &[u64]) -> Result<Self> {
        ctx.check_entry()?;
        let inner = MagnitudePair::from_limbs_in_with(ctx.heap(), limbs, ctx.publishes_gc_owned()).map_err(gc_alloc_error)?;
        Ok(Self::from_pair(inner))
    }

    /// 以显式 heap 容量发布（`capacity >= effective_len`）。
    ///
    /// 供 `*_owned` 复用路径预留余量，以及合同测试构造「capacity > len」的 Heap 值。
    pub fn from_limbs_with_capacity_in(ctx: &NumericContext, limbs: &[u64], capacity: usize) -> Result<Self> {
        use crate::storage::OwnedLimbBuffer;
        ctx.check_entry()?;
        let el = limb_kernel::effective_len(limbs);
        if el <= 2 {
            return Self::from_limb_slice_in(ctx, limbs);
        }
        if capacity < el {
            return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "natural_capacity_too_small"));
        }
        ctx.budget().check_limbs(capacity)?;
        let mut buf = if ctx.publishes_gc_owned() {
            OwnedLimbBuffer::alloc_uninit_gc_owned_in(ctx.heap(), capacity)
        }
        else {
            OwnedLimbBuffer::alloc_uninit_in(ctx.heap(), capacity)
        }
        .map_err(gc_alloc_error)?;
        buf.as_mut_slice(el).copy_from_slice(&limbs[..el]);
        Ok(Self::finish_owned_limbs(buf, el))
    }

    pub(super) fn finish_owned_limbs(buf: crate::storage::OwnedLimbBuffer, el: usize) -> Self {
        match el {
            0 | 1 => {
                let limb = if el == 0 { 0 } else { buf.as_slice(1)[0] };
                drop(buf);
                Self::from_u64(limb)
            }
            2 => {
                let limbs = buf.as_slice(2);
                let pair = [limbs[0], limbs[1]];
                drop(buf);
                Self::from_limb2(pair)
            }
            _ => Self::from_pair(MagnitudePair::from_owned_heap(buf, el)),
        }
    }

    /// Kernel `*_into` 后 canonicalize 并发布（值层 executor，非 machine kernel）。
    pub(crate) fn publish_with_kernel(
        ctx: &NumericContext,
        write: impl FnOnce(
            &mut LimbBuffer,
            &mut crate::kernel::ScratchWorkspace,
            &crate::policy::execution_budget::ExecutionBudget,
        ) -> Result<()>,
    ) -> Result<Self> {
        Self::publish_into(ctx, write)
    }

    /// Kernel `*_into` 后 canonicalize 并发布。
    ///
    /// Living 17 步骤 6：当 [`NumericContext::can_reuse_destination`] 为真时复用 context
    /// 输出 `LimbBuffer` 容量；否则使用临时缓冲。Heap 结果经 GC `OwnedLimbBuffer` 接管。
    pub(super) fn publish_into(
        ctx: &NumericContext,
        write: impl FnOnce(
            &mut LimbBuffer,
            &mut crate::kernel::ScratchWorkspace,
            &crate::policy::execution_budget::ExecutionBudget,
        ) -> Result<()>,
    ) -> Result<Self> {
        ctx.check_entry()?;
        if ctx.can_reuse_destination() {
            ctx.with_out_buf(|out| {
                ctx.with_scratch_frame(|scratch, budget| write(out, scratch, budget))?;
                Self::publish_from_out_buf(ctx, out)
            })
        }
        else {
            let mut out = LimbBuffer::zero();
            ctx.with_scratch_frame(|scratch, budget| write(&mut out, scratch, budget))?;
            Self::from_limb_slice_in(ctx, out.as_canonical())
        }
    }

    /// 将输出缓冲规范 limb 发布到 `ctx` heap，并保留缓冲容量供下次复用。
    fn publish_from_out_buf(ctx: &NumericContext, out: &mut LimbBuffer) -> Result<Self> {
        use crate::storage::OwnedLimbBuffer;
        let el = out.canonical_len();
        if el <= 2 {
            let n = Self::from_limb_slice_in(ctx, out.as_canonical())?;
            let _ = out.set_zero(ctx.budget());
            return Ok(n);
        }
        let limbs = out.as_canonical();
        let mut buf = if ctx.publishes_gc_owned() {
            OwnedLimbBuffer::alloc_uninit_gc_owned_in(ctx.heap(), el)
        }
        else {
            OwnedLimbBuffer::alloc_uninit_in(ctx.heap(), el)
        }
        .map_err(gc_alloc_error)?;
        buf.as_mut_slice(el).copy_from_slice(limbs);
        let _ = out.set_zero(ctx.budget());
        Ok(Self::from_pair(MagnitudePair::from_owned_heap(buf, el)))
    }

    /// 当前 storage mode（供 executor 宽度分派）。
    #[inline]
    pub(crate) fn mode(&self) -> Mode {
        self.inner.mode()
    }

    /// Limb 逻辑长度。
    #[inline]
    pub(crate) fn limb_len(&self) -> usize {
        self.inner.limb_len()
    }

    /// Limb1 载荷。
    #[inline]
    pub(crate) fn limb1(&self) -> Option<u64> {
        self.inner.as_limb1()
    }

    /// Limb2 载荷。
    #[inline]
    pub(crate) fn limb2(&self) -> Option<[u64; 2]> {
        self.inner.as_limb2()
    }

    /// 由 Limb2 构造。
    #[inline]
    pub(crate) fn from_limb2(limbs: [u64; 2]) -> Self {
        Self { inner: MagnitudePair::from_limb2(limbs) }
    }

    /// 固定宽度结果（executor：有效长度 ≤ 2；更长请用 [`Self::from_limb_slice_in`]）。
    #[inline]
    pub(crate) fn from_fixed_limbs(limbs: &[u64]) -> Self {
        Self::from_fixed(limbs)
    }

    /// 由 `u128` 幅度构造。
    #[inline]
    pub(crate) fn from_u128_mag(v: u128) -> Self {
        Self { inner: MagnitudePair::from_u128(v) }
    }

    /// 仅改写内部符号元数据位（[`Natural`] 相等 / 序语义忽略该位）。
    ///
    /// 合同测试用：验证 don't-care 位不进入 `Eq` / `Ord` / `Hash`。
    pub fn with_dont_care_sign_bit(self, negative: bool) -> Self {
        Self::from_pair(self.into_pair().with_negative(negative))
    }

    /// 由物理 pair 构造（**不**清零 sign don't-care 位）。
    pub(crate) fn from_pair(inner: MagnitudePair) -> Self {
        let n = Self { inner };
        #[cfg(debug_assertions)]
        n.debug_assert_invariants();
        n
    }

    /// 取出内部物理 pair（保留 don't-care bits）。
    pub(crate) fn into_pair(self) -> MagnitudePair {
        self.inner
    }

    /// 固定宽度结果回写（有效长度 ≤ 2，不经堆）。
    #[inline]
    fn from_fixed(limbs: &[u64]) -> Self {
        let n = Self { inner: MagnitudePair::from_inline_limbs(limbs) };
        #[cfg(debug_assertions)]
        n.debug_assert_invariants();
        n
    }

    #[cfg(debug_assertions)]
    fn debug_assert_invariants(&self) {
        let limbs = self.as_limbs();
        debug_assert!(!limbs.is_empty(), "Natural as_limbs must expose at least [0]");
        if self.is_zero() {
            debug_assert_eq!(limbs, &[0]);
            debug_assert!(matches!(self.inner.mode(), Mode::Limb1));
        }
        else {
            debug_assert_ne!(*limbs.last().unwrap(), 0);
            match limbs.len() {
                1 => debug_assert!(matches!(self.inner.mode(), Mode::Limb1)),
                2 => debug_assert!(matches!(self.inner.mode(), Mode::Limb2)),
                n => {
                    debug_assert!(n >= 3);
                    debug_assert!(matches!(self.inner.mode(), Mode::Heap));
                }
            }
        }
    }
}
