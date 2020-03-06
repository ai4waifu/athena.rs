//! [`Natural`] 消费 `self` 的 destination-reuse 算术入口。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::Natural;
use crate::{kernel::limb as limb_kernel, policy::execution_budget::NumericContext, storage::Mode};

impl Natural {
    /// 消费 `self` 的加法：Heap 且 `capacity >= max(len)+1` 时经 [`MagnitudePair::try_reuse_unique_buffer`] 就地复用。
    ///
    /// 容量不足或非 Heap 时回退 [`Self::try_add`]。`rhs` 与 `self` 同缓冲（含自加）时先拷贝右操作数 limb。
    pub fn try_add_owned(mut self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if rhs.is_zero() {
            return Ok(self);
        }
        if self.is_zero() {
            return rhs.try_clone_in(ctx);
        }

        let la = self.limb_len();
        let lb = rhs.limb_len();
        let need = la.max(lb) + 1;
        ctx.budget().check_limbs(need)?;

        let can_steal = matches!(self.inner.mode(), Mode::Heap) && self.inner.heap_capacity().is_some_and(|c| c >= need);
        if !can_steal {
            return self.try_add(rhs, ctx);
        }

        let rhs_tmp;
        let rb: &[u64] = {
            let overlap = match (self.inner.heap_ptr(), rhs.inner.heap_ptr()) {
                (Some(sp), Some(rp)) => sp == rp,
                _ => false,
            };
            if overlap {
                rhs_tmp = rhs.as_limbs().to_vec();
                rhs_tmp.as_slice()
            }
            else {
                rhs.as_limbs()
            }
        };
        let lb = limb_kernel::effective_len(rb);
        let n = la.max(lb);
        let need = n + 1;
        debug_assert!(self.inner.heap_capacity().is_some_and(|c| c >= need));

        // Living 31：TracingSweep 持久块暂不可 steal（待 UniqueMutationGuard）。失败回退 try_add。
        let Some(mut buf) = self.inner.try_reuse_unique_buffer()
        else {
            return self.try_add(rhs, ctx);
        };
        {
            let storage = buf.as_mut_slice(need);
            // 超出原有效长度的槽位可能未初始化，就地 adc 前必须置零。
            for slot in &mut storage[la..] {
                *slot = 0;
            }
            let mut carry = 0u64;
            for i in 0..n {
                let (sum, c) = limb_kernel::adc(storage[i], *rb.get(i).unwrap_or(&0), carry);
                storage[i] = sum;
                carry = c;
            }
            storage[n] = carry;
        }
        let el = {
            let storage = buf.as_slice(need);
            if storage[n] != 0 { n + 1 } else { limb_kernel::effective_len(&storage[..n]) }
        };
        Ok(Self::finish_owned_limbs(buf, el))
    }

    /// 消费 `self` 的 `× u64`：Heap 且 `capacity >= len+1` 时就地 `mul_1`。
    pub fn try_mul_u64_owned(mut self, rhs: u64, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if self.is_zero() || rhs == 0 {
            return Ok(Self::zero());
        }
        if rhs == 1 {
            return Ok(self);
        }

        let la = self.limb_len();
        let need = la + 1;
        ctx.budget().check_limbs(need)?;

        let can_steal = matches!(self.inner.mode(), Mode::Heap) && self.inner.heap_capacity().is_some_and(|c| c >= need);
        if !can_steal {
            return self.try_mul_u64(rhs, ctx);
        }

        let Some(mut buf) = self.inner.try_reuse_unique_buffer()
        else {
            return self.try_mul_u64(rhs, ctx);
        };
        {
            let storage = buf.as_mut_slice(need);
            storage[la] = 0;
            let mut carry = 0u128;
            for i in 0..la {
                let prod = u128::from(storage[i]) * u128::from(rhs) + carry;
                storage[i] = prod as u64;
                carry = prod >> 64;
            }
            storage[la] = carry as u64;
        }
        let el = {
            let storage = buf.as_slice(need);
            if storage[la] != 0 { la + 1 } else { la }
        };
        Ok(Self::finish_owned_limbs(buf, el))
    }
    /// 消费 `self` 的减法：Heap 且容量足够时就地 SBB（要求 `self >= rhs`）。
    pub fn try_sub_owned(mut self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if rhs.is_zero() {
            return Ok(self);
        }
        if limb_kernel::cmp_slice(self.as_limbs(), rhs.as_limbs()).is_lt() {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "numeric")
                .detail("operation", "natural_sub")
                .detail("reason", "underflow"));
        }

        let la = self.limb_len();
        let lb = rhs.limb_len();
        let need = la.max(lb);
        ctx.budget().check_limbs(need)?;

        let can_steal = matches!(self.inner.mode(), Mode::Heap) && self.inner.heap_capacity().is_some_and(|c| c >= need);
        if !can_steal {
            return self.try_sub(rhs, ctx);
        }

        let overlap = match (self.inner.heap_ptr(), rhs.inner.heap_ptr()) {
            (Some(sp), Some(rp)) => sp == rp,
            _ => false,
        };
        if overlap {
            // self - self = 0
            return Ok(Self::zero());
        }

        let rb = rhs.as_limbs();
        let lb = limb_kernel::effective_len(rb);
        let n = la.max(lb);

        let Some(mut buf) = self.inner.try_reuse_unique_buffer()
        else {
            return self.try_sub(rhs, ctx);
        };
        {
            let storage = buf.as_mut_slice(n.max(1));
            let mut borrow = 0u64;
            for i in 0..n {
                let (diff, b_out) = limb_kernel::sbb(storage[i], *rb.get(i).unwrap_or(&0), borrow);
                storage[i] = diff;
                borrow = b_out;
            }
            debug_assert_eq!(borrow, 0);
        }
        let el = limb_kernel::effective_len(buf.as_slice(n.max(1))).max(1);
        Ok(Self::finish_owned_limbs(buf, el))
    }
    /// 消费 `self` 的乘法：仅在 planner 选 Schoolbook 且 Heap 容量 ≥ `la+lb` 时 `try_reuse_unique_buffer`。
    ///
    /// 更宽路径（Karatsuba/Toom）回退 [`Self::try_mul`]，避免就地清零破坏输入。
    pub fn try_mul_owned(mut self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if self.is_zero() || rhs.is_zero() {
            return Ok(Self::zero());
        }
        if rhs.is_one() {
            return Ok(self);
        }
        if self.is_one() {
            return rhs.try_clone_in(ctx);
        }

        let la = self.limb_len();
        let lb = rhs.limb_len();
        let need = la + lb;
        ctx.budget().check_limbs(need)?;

        let plan = ctx.planner().plan_mul(la, lb);
        let schoolbook = matches!(plan, crate::algorithm::MulStrategy::Schoolbook);
        let can_steal = schoolbook && matches!(self.inner.mode(), Mode::Heap) && self.inner.heap_capacity().is_some_and(|c| c >= need);
        if !can_steal {
            return self.try_mul(rhs, ctx);
        }

        // Snapshot lhs before clearing destination (schoolbook zeros out).
        let lhs_snap = self.as_limbs()[..la].to_vec();
        let rhs_tmp;
        let rb: &[u64] = {
            let overlap = match (self.inner.heap_ptr(), rhs.inner.heap_ptr()) {
                (Some(sp), Some(rp)) => sp == rp,
                _ => false,
            };
            if overlap {
                rhs_tmp = lhs_snap.clone();
                rhs_tmp.as_slice()
            }
            else {
                rhs.as_limbs()
            }
        };
        let lb = limb_kernel::effective_len(rb);

        let Some(mut buf) = self.inner.try_reuse_unique_buffer()
        else {
            return self.try_mul(rhs, ctx);
        };
        {
            let storage = buf.as_mut_slice(need);
            limb_kernel::mul_schoolbook_into(&lhs_snap, &rb[..lb], storage);
        }
        let el = limb_kernel::effective_len(buf.as_slice(need)).max(1);
        Ok(Self::finish_owned_limbs(buf, el))
    }
}
