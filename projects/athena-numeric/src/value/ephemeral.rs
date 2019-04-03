//! Batch-local ephemeral magnitudes（绑定 [`NumericBatch`] 借用期）。
//!
//! `EphemeralNatural` / `EphemeralInteger` 的结果指针在 batch `finish` / Drop 后失效。
//! 跨 batch 存活必须 [`EphemeralNatural::promote`] / [`EphemeralInteger::promote`]。
//! **禁止**把本模块类型伪装成长期 [`super::natural::Natural`] / [`super::integer::Integer`]。

use core::marker::PhantomData;

use athena_gc::{GcHeap, NumericBatch};
use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    kernel::limb as limb_kernel,
    policy::execution_budget::NumericContext,
    storage::{MagnitudePair, OwnedLimbBuffer, gc_alloc_error},
    value::{
        integer::{Integer, Sign},
        natural::Natural,
    },
};

/// 批内非负幅度：生命周期绑在对 [`NumericBatch`] 的独占借用上。
///
/// Heap 模式结果位于 ephemeral bump；不得逃逸 batch，除非 [`Self::promote`]。
pub struct EphemeralNatural<'batch> {
    inner: MagnitudePair,
    _batch: PhantomData<&'batch mut ()>,
}

/// 批内有符号整数（同 [`EphemeralNatural`] 生命周期合同）。
pub struct EphemeralInteger<'batch> {
    inner: MagnitudePair,
    _batch: PhantomData<&'batch mut ()>,
}

impl<'batch> EphemeralNatural<'batch> {
    fn from_pair(inner: MagnitudePair) -> Self {
        Self { inner, _batch: PhantomData }
    }

    /// 小端加法，结果发布到 `batch` 独占 heap。
    pub fn try_add(lhs: &[u64], rhs: &[u64], batch: &'batch mut NumericBatch<'_>) -> Result<Self> {
        publish_add_slices_mut(batch.heap_mut(), lhs, rhs).map(Self::from_pair)
    }

    /// 小端减法（`lhs >= rhs`），结果发布到 `batch`。
    pub fn try_sub(lhs: &[u64], rhs: &[u64], batch: &'batch mut NumericBatch<'_>) -> Result<Self> {
        publish_sub_slices_mut(batch.heap_mut(), lhs, rhs).map(Self::from_pair)
    }

    /// 小学乘法，结果发布到 `batch`。
    pub fn try_mul_schoolbook(lhs: &[u64], rhs: &[u64], batch: &'batch mut NumericBatch<'_>) -> Result<Self> {
        publish_mul_schoolbook_mut(batch.heap_mut(), lhs, rhs).map(Self::from_pair)
    }

    /// 借用小端 limb（不得长过 `&self`，也不得长过 batch）。
    #[inline]
    pub fn as_limbs(&self) -> &[u64] {
        self.inner.as_limbs()
    }

    /// 是否为零。
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// 拷贝到 `ctx` 持久 heap，得到可逃逸的 [`Natural`]。
    ///
    /// `ctx.heap` **不得**是当前 batch 正在 rewind 的同一 heap，否则 `finish` 会抹掉拷贝。
    pub fn promote(&self, ctx: &NumericContext) -> Result<Natural> {
        Natural::from_limb_slice_in(ctx, self.as_limbs())
    }
}

impl<'batch> EphemeralInteger<'batch> {
    fn from_pair(inner: MagnitudePair) -> Self {
        Self { inner, _batch: PhantomData }
    }

    /// 同号相加 / 异号相减的批内加法（幅度借用）。
    pub fn try_add(
        lhs_limbs: &[u64],
        lhs_neg: bool,
        rhs_limbs: &[u64],
        rhs_neg: bool,
        batch: &'batch mut NumericBatch<'_>,
    ) -> Result<Self> {
        let lz = limb_kernel::is_zero(lhs_limbs);
        let rz = limb_kernel::is_zero(rhs_limbs);
        if lz && rz {
            return Ok(Self::from_pair(MagnitudePair::zero()));
        }
        if lz {
            let mut p = publish_from_limbs_mut(batch.heap_mut(), rhs_limbs)?;
            if rhs_neg && !limb_kernel::is_zero(p.as_limbs()) {
                p.set_sign_bit(true);
            }
            return Ok(Self::from_pair(p));
        }
        if rz {
            let mut p = publish_from_limbs_mut(batch.heap_mut(), lhs_limbs)?;
            if lhs_neg {
                p.set_sign_bit(true);
            }
            return Ok(Self::from_pair(p));
        }
        if lhs_neg == rhs_neg {
            let mut p = publish_add_slices_mut(batch.heap_mut(), lhs_limbs, rhs_limbs)?;
            if lhs_neg {
                p.set_sign_bit(true);
            }
            Ok(Self::from_pair(p))
        }
        else {
            match limb_kernel::cmp_slice(lhs_limbs, rhs_limbs) {
                core::cmp::Ordering::Equal => Ok(Self::from_pair(MagnitudePair::zero())),
                core::cmp::Ordering::Greater => {
                    let mut p = publish_sub_slices_mut(batch.heap_mut(), lhs_limbs, rhs_limbs)?;
                    if lhs_neg {
                        p.set_sign_bit(true);
                    }
                    Ok(Self::from_pair(p))
                }
                core::cmp::Ordering::Less => {
                    let mut p = publish_sub_slices_mut(batch.heap_mut(), rhs_limbs, lhs_limbs)?;
                    if rhs_neg {
                        p.set_sign_bit(true);
                    }
                    Ok(Self::from_pair(p))
                }
            }
        }
    }

    /// 借用小端 limb。
    #[inline]
    pub fn as_limbs(&self) -> &[u64] {
        self.inner.as_limbs()
    }

    /// 符号。
    #[inline]
    pub fn sign(&self) -> Sign {
        if self.inner.is_zero() {
            Sign::Zero
        }
        else if self.inner.is_negative() {
            Sign::Negative
        }
        else {
            Sign::Positive
        }
    }

    /// 是否为零。
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// 拷贝到 `ctx` 持久 heap。
    ///
    /// `ctx.heap` **不得**是当前 batch 正在 rewind 的同一 heap。
    pub fn promote(&self, ctx: &NumericContext) -> Result<Integer> {
        let n = Integer::from_limbs_in(ctx, self.as_limbs())?;
        Ok(if self.inner.is_negative() { n.neg() } else { n })
    }
}

fn finish_pair(buf: OwnedLimbBuffer, el: usize) -> MagnitudePair {
    match el {
        0 | 1 => {
            let limb = if el == 0 { 0 } else { buf.as_slice(1)[0] };
            drop(buf);
            MagnitudePair::from_u64(limb)
        }
        2 => {
            let limbs = buf.as_slice(2);
            let pair = [limbs[0], limbs[1]];
            drop(buf);
            MagnitudePair::from_limb2(pair)
        }
        _ => MagnitudePair::from_owned_heap(buf, el),
    }
}

fn publish_from_limbs_mut(heap: &mut GcHeap, limbs: &[u64]) -> Result<MagnitudePair> {
    let el = limb_kernel::effective_len(limbs);
    if el == 0 {
        return Ok(MagnitudePair::zero());
    }
    if el <= 2 {
        return Ok(MagnitudePair::from_inline_limbs(&limbs[..el]));
    }
    let mut buf = OwnedLimbBuffer::alloc_uninit_mut(heap, el).map_err(gc_alloc_error)?;
    buf.as_mut_slice(el).copy_from_slice(&limbs[..el]);
    Ok(finish_pair(buf, el))
}

fn publish_add_slices_mut(heap: &mut GcHeap, a: &[u64], b: &[u64]) -> Result<MagnitudePair> {
    let la = limb_kernel::effective_len(a);
    let lb = limb_kernel::effective_len(b);
    if la == 0 {
        return publish_from_limbs_mut(heap, b);
    }
    if lb == 0 {
        return publish_from_limbs_mut(heap, a);
    }
    let n = la.max(lb);
    let capacity = n + 1;
    let mut buf = OwnedLimbBuffer::alloc_uninit_mut(heap, capacity).map_err(gc_alloc_error)?;
    let storage = buf.as_mut_slice(capacity);
    let mut carry = 0u64;
    for i in 0..n {
        let (sum, c) = limb_kernel::adc(*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0), carry);
        storage[i] = sum;
        carry = c;
    }
    storage[n] = carry;
    let el = if carry != 0 { n + 1 } else { limb_kernel::effective_len(&storage[..n]) };
    Ok(finish_pair(buf, el))
}

fn publish_sub_slices_mut(heap: &mut GcHeap, a: &[u64], b: &[u64]) -> Result<MagnitudePair> {
    if limb_kernel::cmp_slice(a, b) == core::cmp::Ordering::Less {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "numeric")
            .detail("operation", "ephemeral_sub")
            .detail("reason", "underflow"));
    }
    let n = limb_kernel::effective_len(a);
    if n == 0 {
        return Ok(MagnitudePair::zero());
    }
    let mut buf = OwnedLimbBuffer::alloc_uninit_mut(heap, n).map_err(gc_alloc_error)?;
    let storage = buf.as_mut_slice(n);
    let mut borrow = 0u64;
    for i in 0..n {
        let (diff, b_out) = limb_kernel::sbb(*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0), borrow);
        storage[i] = diff;
        borrow = b_out;
    }
    debug_assert_eq!(borrow, 0);
    let el = limb_kernel::effective_len(storage).max(1);
    Ok(finish_pair(buf, el))
}

fn publish_mul_schoolbook_mut(heap: &mut GcHeap, a: &[u64], b: &[u64]) -> Result<MagnitudePair> {
    let la = limb_kernel::effective_len(a);
    let lb = limb_kernel::effective_len(b);
    if la == 0 || lb == 0 {
        return Ok(MagnitudePair::zero());
    }
    let capacity = la + lb;
    let mut buf = OwnedLimbBuffer::alloc_uninit_mut(heap, capacity).map_err(gc_alloc_error)?;
    let storage = buf.as_mut_slice(capacity);
    storage.fill(0);
    limb_kernel::mul_schoolbook_into(&a[..la], &b[..lb], storage);
    let el = limb_kernel::effective_len(storage).max(1);
    Ok(finish_pair(buf, el))
}

#[cfg(test)]
mod tests {
    use athena_gc::{GcHeap, HeapBudget};

    use crate::policy::execution_budget::{ExecutionBudget, NumericContext};

    use super::EphemeralNatural;

    #[test]
    fn ephemeral_natural_batch_lease_and_promote() {
        let batch_heap = GcHeap::new_shared(HeapBudget::default());
        let persist_heap = GcHeap::new_shared(HeapBudget::default());
        let persist_ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), persist_heap.clone());
        let mut h = batch_heap.borrow_mut();
        let used0: usize = h.segments().filter(|s| s.kind == athena_gc::SegmentKind::Numeric).map(|s| s.used).sum();
        let mut promoted = None;
        h.with_numeric_batch(|batch| {
            for _ in 0..16 {
                let block = batch.allocate_limbs(4).expect("alloc");
                let _ = block;
            }
            let n = EphemeralNatural::try_add(&[1, 2, 3, 4], &[5, 6, 7, 8], batch).expect("add");
            assert!(!n.is_zero());
            promoted = Some(n.promote(&persist_ctx).expect("promote"));
            drop(n);
        })
        .expect("batch");
        let used1: usize = h.segments().filter(|s| s.kind == athena_gc::SegmentKind::Numeric).map(|s| s.used).sum();
        assert_eq!(used1, used0, "batch rewind restores bump");
        assert_eq!(h.accounting(), athena_gc::AllocationAccounting::Full);
        assert!(!h.bump_ephemeral());
        let p = promoted.expect("promoted");
        assert_eq!(p.as_limbs(), &[6, 8, 10, 12]);
        assert!(persist_heap.borrow().resident_bytes() > 0);
    }
}
