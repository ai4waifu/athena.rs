//! 非负大整数（`meta` + 纯 `union Magnitude`；算法委托 [`crate::kernel::limb`]）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    kernel::{LimbBuffer, limb as limb_kernel},
    policy::execution_budget::NumericContext,
    storage::{MagnitudePair, Mode, gc_alloc_error},
};
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    str::FromStr,
};

/// 自然数（小端 `u64` limb，无尾随零）。
///
/// 布局：`meta`（mode+heap_len；sign 位 don't-care）+ `union Magnitude`，LP64 上 24 bytes。
/// 经私有 [`MagnitudePair`] 做 Drop/Clone；读取时不解释 sign，语义恒为非负。
///
/// # Clone
///
/// Limb1 / Limb2 栈拷贝。Heap `GcOwned`（Session 发布）经 `NumericRoot` 共享，不分配 limb。
/// Heap `RustOwned` 会同堆再分配；失败时 **panic**（债）。算术热路径应借用 limb，
/// owning 复制用 [`Self::try_clone_in`]。
#[derive(Clone, Default)]
pub struct Natural {
    inner: MagnitudePair,
}

impl PartialEq for Natural {
    fn eq(&self, other: &Self) -> bool {
        // 忽略 meta sign/reserved don't-care 位。
        self.as_limbs() == other.as_limbs()
    }
}

impl Eq for Natural {}

impl Hash for Natural {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_limbs().hash(state);
    }
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<Natural>() == 24);
    assert!(core::mem::align_of::<Natural>() == 8);
};

impl core::fmt::Debug for Natural {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Natural").field("limbs", &self.as_limbs()).finish()
    }
}

impl PartialOrd for Natural {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Natural {
    fn cmp(&self, other: &Self) -> Ordering {
        limb_kernel::cmp_slice(self.as_limbs(), other.as_limbs())
    }
}

impl Natural {
    /// 零。
    pub fn zero() -> Self {
        Self { inner: MagnitudePair::zero() }
    }

    /// 一。
    pub fn one() -> Self {
        Self { inner: MagnitudePair::from_u64(1) }
    }

    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// 是否为一。
    pub fn is_one(&self) -> bool {
        matches!(self.inner.mode(), Mode::Limb1) && self.as_limbs() == [1]
    }

    /// 最低 limb 是否为奇数。
    pub fn is_odd(&self) -> bool {
        !self.is_zero() && (self.as_limbs()[0] & 1) == 1
    }

    /// 由 `u64` 构造。
    pub fn from_u64(n: u64) -> Self {
        Self { inner: MagnitudePair::from_u64(n) }
    }

    /// 二进制位宽（零 → 0）。
    pub fn bits(&self) -> u64 {
        if self.is_zero() {
            return 0;
        }
        let limbs = self.as_limbs();
        let top = limbs.len() - 1;
        (top as u64) * 64 + (64 - limbs[top].leading_zeros() as u64)
    }

    /// 右移一位（整除 2）。
    pub fn div2(&mut self) {
        if self.is_zero() {
            return;
        }
        let mut limbs = self.as_limbs().to_vec();
        let mut carry = 0u64;
        let len = limb_kernel::effective_len(&limbs);
        for i in (0..len).rev() {
            let limb = limbs[i];
            let new_carry = limb & 1;
            limbs[i] = (limb >> 1) | (carry << 63);
            carry = new_carry;
        }
        *self = Self::from_limbs(limb_kernel::normalize_trim(limbs)).expect("gc numeric alloc");
    }

    /// 右移 `n` 位（丢弃低位）。
    pub(crate) fn shr_bits(&self, n: u64) -> Self {
        if n == 0 || self.is_zero() {
            return self.clone();
        }
        let bits = self.bits();
        if n >= bits {
            return Self::zero();
        }
        let limb_shift = (n / 64) as usize;
        let bit_shift = (n % 64) as u32;
        let src = self.as_limbs();
        let el = src.len();
        if limb_shift >= el {
            return Self::zero();
        }
        let out_len = el - limb_shift;
        let mut out = vec![0u64; out_len];
        for i in 0..out_len {
            let src_i = i + limb_shift;
            out[i] = src[src_i] >> bit_shift;
            if bit_shift != 0 && src_i + 1 < el {
                out[i] |= src[src_i + 1] << (64 - bit_shift);
            }
        }
        Self::from_limbs(out).expect("gc numeric alloc")
    }

    /// 测试第 `index` 位（0 = LSB）。越界为 false。
    pub(crate) fn bit(&self, index: u64) -> bool {
        let limb_i = (index / 64) as usize;
        let limbs = self.as_limbs();
        if limb_i >= limbs.len() {
            return false;
        }
        let bit_i = (index % 64) as u32;
        (limbs[limb_i] >> bit_i) & 1 == 1
    }

    /// 是否有低于 `bit_index` 的任意置位（即 bits `[0, bit_index)`）。
    pub(crate) fn any_bits_below(&self, bit_index: u64) -> bool {
        if bit_index == 0 || self.is_zero() {
            return false;
        }
        let full_limbs = (bit_index / 64) as usize;
        let rem_bits = (bit_index % 64) as u32;
        let limbs = self.as_limbs();
        let el = limbs.len();
        let scan = full_limbs.min(el);
        for &limb in &limbs[..scan] {
            if limb != 0 {
                return true;
            }
        }
        if rem_bits > 0 && full_limbs < el {
            let mask = (1u64 << rem_bits) - 1;
            if limbs[full_limbs] & mask != 0 {
                return true;
            }
        }
        false
    }

    /// 加小整数（默认 [`NumericContext::portable_default`]）。
    pub fn add_u64(&self, rhs: u64) -> Self {
        self.try_add_u64(rhs, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 加小整数（服从 `ctx` 预算）。
    pub fn try_add_u64(&self, rhs: u64, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if rhs == 0 {
            return self.try_clone_in(ctx);
        }
        match self.inner.mode() {
            Mode::Limb1 if self.is_zero() => {
                ctx.budget().check_limbs(1)?;
                Ok(Self::from_u64(rhs))
            }
            Mode::Limb1 => {
                let a = self.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                let (lo, carry) = limb_kernel::add_1(a, rhs);
                Ok(if carry == 0 { Self::from_u64(lo) } else { Self { inner: MagnitudePair::from_limb2([lo, 1]) } })
            }
            Mode::Limb2 => {
                let a = self.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_1_2(rhs, a);
                Self::from_limb_slice_in(ctx, &limbs[..len])
            }
            Mode::Heap => Self::publish_into(ctx, |out, scratch, budget| {
                ctx.kernels().add_into(ctx.kernel_token(), self.as_limbs(), &[rhs], out, scratch, budget)
            }),
        }
    }

    /// 乘小整数（默认 [`NumericContext::portable_default`]）。
    pub fn mul_u64(&self, rhs: u64) -> Self {
        self.try_mul_u64(rhs, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 乘小整数（服从 `ctx` 预算）。
    pub fn try_mul_u64(&self, rhs: u64, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if self.is_zero() || rhs == 0 {
            return Ok(Self::zero());
        }
        if rhs == 1 {
            return self.try_clone_in(ctx);
        }
        match self.inner.mode() {
            Mode::Limb1 => {
                let a = self.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                Ok(Self { inner: MagnitudePair::from_u128(limb_kernel::mul_1x1(a, rhs)) })
            }
            Mode::Limb2 => {
                let a = self.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::mul_2x1(a, rhs);
                Self::from_limb_slice_in(ctx, &limbs[..len])
            }
            Mode::Heap => Self::publish_into(ctx, |out, scratch, budget| {
                ctx.kernels().mul_1_into(ctx.kernel_token(), self.as_limbs(), rhs, out, scratch, budget)
            }),
        }
    }

    /// 加法（默认 [`NumericContext::portable_default`]）。
    pub fn add(&self, rhs: &Self) -> Self {
        self.try_add(rhs, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 加法（服从 `ctx` 预算）。
    pub fn try_add(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        crate::dispatch::NumericExecutor::add_natural(self, rhs, ctx)
    }

    /// 消费 `self` 的加法：Heap 且 `capacity >= max(len)+1` 时经 [`MagnitudePair::steal_heap`] 就地复用。
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

        let mut buf = self.inner.steal_heap().expect("Heap capacity checked");
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

        let mut buf = self.inner.steal_heap().expect("Heap capacity checked");
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

    /// 减法（要求 `self >= rhs`；默认上下文）。
    pub fn sub(&self, rhs: &Self) -> Self {
        self.try_sub(rhs, &NumericContext::portable_default()).expect("natural sub precondition or unbounded default")
    }

    /// 减法（`self >= rhs`；服从 `ctx` 预算）。
    pub fn try_sub(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        crate::dispatch::NumericExecutor::sub_natural(self, rhs, ctx)
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

        let mut buf = self.inner.steal_heap().expect("Heap capacity checked");
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

    /// 乘法（默认 [`NumericContext::portable_default`]）。
    pub fn mul(&self, rhs: &Self) -> Self {
        self.try_mul(rhs, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 乘法（服从 `ctx` 预算）。
    pub fn try_mul(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        crate::dispatch::NumericExecutor::mul_natural(self, rhs, ctx)
    }

    /// 消费 `self` 的乘法：仅在 planner 选 Schoolbook 且 Heap 容量 ≥ `la+lb` 时 `steal_heap`。
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
        let can_steal =
            schoolbook && matches!(self.inner.mode(), Mode::Heap) && self.inner.heap_capacity().is_some_and(|c| c >= need);
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

        let mut buf = self.inner.steal_heap().expect("Heap capacity checked");
        {
            let storage = buf.as_mut_slice(need);
            limb_kernel::mul_schoolbook_into(&lhs_snap, &rb[..lb], storage);
        }
        let el = limb_kernel::effective_len(buf.as_slice(need)).max(1);
        Ok(Self::finish_owned_limbs(buf, el))
    }

    /// 平方（默认 [`NumericContext::portable_default`]）。
    pub fn sqr(&self) -> Self {
        self.try_sqr(&NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 平方（服从 `ctx` 预算）。
    pub fn try_sqr(&self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if self.is_zero() {
            return Ok(Self::zero());
        }
        match self.inner.mode() {
            Mode::Limb1 => {
                let a = self.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                Ok(Self { inner: MagnitudePair::from_u128(limb_kernel::mul_1x1(a, a)) })
            }
            Mode::Limb2 => {
                let a = self.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(4)?;
                let (limbs, len) = limb_kernel::mul_2(a, a);
                Self::from_limb_slice_in(ctx, &limbs[..len])
            }
            Mode::Heap => {
                let plan = ctx.planner().plan_mul(self.limb_len(), self.limb_len());
                Self::publish_into(ctx, |out, scratch, budget| {
                    ctx.kernels().sqr_into(ctx.kernel_token(), self.as_limbs(), plan, out, scratch, budget)
                })
            }
        }
    }

    /// 除法与余数（`rhs > 0`；默认上下文）。
    pub fn div_rem(&self, rhs: &Self) -> (Self, Self) {
        self.try_div_rem(rhs, &NumericContext::portable_default()).expect("div_rem divisor non-zero and unbounded default")
    }

    /// 除法与余数（服从 `ctx` 预算；除数为零返回诊断）。
    pub fn try_div_rem(&self, rhs: &Self, ctx: &NumericContext) -> Result<(Self, Self)> {
        Self::try_div_rem_limbs(self.as_limbs(), rhs.as_limbs(), ctx)
    }

    /// 借用 limb 除法与余数；结果发布到 `ctx`。
    pub fn try_div_rem_limbs(lhs: &[u64], rhs: &[u64], ctx: &NumericContext) -> Result<(Self, Self)> {
        crate::dispatch::NumericExecutor::div_rem_limbs(lhs, rhs, ctx)
    }

    /// 模幂（`modulus > 0`；默认上下文）。
    pub fn mod_pow(&self, exp: &Self, modulus: &Self) -> Self {
        self.try_mod_pow(exp, modulus, &NumericContext::portable_default())
            .expect("mod_pow modulus non-zero and unbounded default")
    }

    /// 模幂（服从 `ctx` 预算；奇模数足够宽时走 Montgomery）。
    pub fn try_mod_pow(&self, exp: &Self, modulus: &Self, ctx: &NumericContext) -> Result<Self> {
        Self::try_mod_pow_limbs(self.as_limbs(), exp.as_limbs(), modulus.as_limbs(), ctx)
    }

    /// 借用 limb 模幂；结果发布到 `ctx`。
    pub fn try_mod_pow_limbs(base: &[u64], exp: &[u64], modulus: &[u64], ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if limb_kernel::is_zero(modulus) || modulus.is_empty() {
            return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
                .detail("domain", "numeric")
                .detail("operation", "natural_mod_pow")
                .detail("reason", "modulus_zero"));
        }
        let mod_len = limb_kernel::effective_len(modulus);
        ctx.budget().check_limbs(mod_len)?;
        if mod_len == 1 && modulus[0] == 1 {
            return Ok(Self::zero());
        }
        let modulus = &modulus[..mod_len];
        if limb_kernel::mod_pow_montgomery_eligible(modulus) {
            return Self::from_limbs_in(ctx, limb_kernel::mod_pow_montgomery(base, exp, modulus));
        }
        let mut result = Self::one();
        let mut base_n = Self::from_limb_slice_in(ctx, base)?;
        let mut e = Self::from_limb_slice_in(ctx, exp)?;
        let modulus_n = Self::from_limb_slice_in(ctx, modulus)?;
        base_n = base_n.try_div_rem(&modulus_n, ctx)?.1;
        while !e.is_zero() {
            if e.is_odd() {
                result = result.try_mul(&base_n, ctx)?.try_div_rem(&modulus_n, ctx)?.1;
            }
            base_n = base_n.try_sqr(ctx)?.try_div_rem(&modulus_n, ctx)?.1;
            e.div2();
        }
        Ok(result)
    }

    /// 是否为 2 的幂（正整数）。
    pub fn is_power_of_two(&self) -> bool {
        if self.is_zero() {
            return false;
        }
        let mut ones = 0u32;
        for &limb in self.as_limbs() {
            ones += limb.count_ones();
            if ones > 1 {
                return false;
            }
        }
        ones == 1
    }

    /// 十进制字符串（无符号）。
    ///
    /// Living `19`：只借用 limb 并在本地 `Vec` 上辗转相除，**不** `Clone` / 不登记 root / 不走 GC 分配。
    pub fn to_decimal_string(&self) -> String {
        Self::decimal_from_limbs(self.as_limbs())
    }

    /// 由小端 limb 生成无符号十进制（观察者路径；无 owning 数值复制）。
    pub(crate) fn decimal_from_limbs(limbs: &[u64]) -> String {
        if limbs.is_empty() || limb_kernel::is_zero(limbs) {
            return "0".to_string();
        }
        let mut buf: Vec<u64> = limbs.to_vec();
        let mut digits = Vec::new();
        // `limb_kernel::is_zero` 对空切片为 false；除尽后须保留 `[0]` 或显式判空，避免死循环。
        loop {
            if buf.is_empty() || limb_kernel::is_zero(&buf) {
                break;
            }
            let mut rem = 0u128;
            for limb in buf.iter_mut().rev() {
                let cur = (rem << 64) | u128::from(*limb);
                *limb = (cur / 10) as u64;
                rem = cur % 10;
            }
            digits.push(b'0' + rem as u8);
            while buf.len() > 1 && buf.last() == Some(&0) {
                buf.pop();
            }
        }
        if digits.is_empty() {
            return "0".to_string();
        }
        digits.reverse();
        String::from_utf8(digits).expect("ASCII digit bytes are UTF-8")
    }

    /// 可落入 `u64` 时返回（零 → `Some(0)`）。
    pub fn to_u64(&self) -> Option<u64> {
        if self.is_zero() {
            return Some(0);
        }
        let limbs = self.as_limbs();
        if limbs.len() == 1 { Some(limbs[0]) } else { None }
    }

    /// 可落入 `u128` 时返回。
    pub fn to_u128(&self) -> Option<u128> {
        if self.is_zero() {
            return Some(0);
        }
        let limbs = self.as_limbs();
        match limbs.len() {
            1 => Some(limbs[0] as u128),
            2 => Some(limbs[0] as u128 | ((limbs[1] as u128) << 64)),
            _ => None,
        }
    }

    /// 非负最大公约数（默认 [`NumericContext::portable_default`]）。
    pub fn gcd(&self, other: &Self) -> Self {
        self.try_gcd(other, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 非负最大公约数（服从 `ctx` 预算；结果 limb ≤ `min(self, other)`）。
    pub fn try_gcd(&self, other: &Self, ctx: &NumericContext) -> Result<Self> {
        Self::try_gcd_limbs(self.as_limbs(), other.as_limbs(), ctx)
    }

    /// 借用 limb 最大公约数；结果发布到 `ctx`。
    pub fn try_gcd_limbs(lhs: &[u64], rhs: &[u64], ctx: &NumericContext) -> Result<Self> {
        crate::dispatch::NumericExecutor::gcd_limbs(lhs, rhs, ctx)
    }

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

    /// 以显式 heap 容量发布（`capacity >= effective_len`）。供 `*_owned` 复用与测试构造余量缓冲。
    pub(crate) fn from_limbs_with_capacity_in(ctx: &NumericContext, limbs: &[u64], capacity: usize) -> Result<Self> {
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

    fn finish_owned_limbs(buf: crate::storage::OwnedLimbBuffer, el: usize) -> Self {
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
    fn publish_into(
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

    /// 二进制 wire 幅度：`u32` 小端 limb 计数 + `u64` 小端 limb。
    ///
    /// 零编码为 `count=1` 且 limb `0`。解码拒绝 `count=0` 与尾随零 limb。
    pub(crate) fn wire_encode_magnitude(&self) -> Vec<u8> {
        let limbs = self.as_limbs();
        let el = limbs.len();
        let mut out = Vec::with_capacity(4 + el * 8);
        out.extend_from_slice(&(el as u32).to_le_bytes());
        for &limb in limbs {
            out.extend_from_slice(&limb.to_le_bytes());
        }
        out
    }

    /// 在执行预算下解码 [`Self::wire_encode_magnitude`] 字节（canonical reject）。
    pub(crate) fn wire_decode_magnitude_budgeted(
        bytes: &[u8],
        budget: &crate::policy::execution_budget::ExecutionBudget,
    ) -> Result<Self> {
        use crate::format::validation::assert_canonical_magnitude_limbs;
        if bytes.len() < 4 {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_short"));
        }
        budget.check_wire_bytes(bytes.len())?;
        let count = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| {
            Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_count")
        })?) as usize;
        budget.check_limbs(count)?;
        let need = 4usize
            .checked_add(count.checked_mul(8).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_overflow")
            })?)
            .ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_overflow")
            })?;
        if bytes.len() != need {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_len"));
        }
        let mut limbs = Vec::with_capacity(count);
        for i in 0..count {
            let off = 4 + i * 8;
            limbs.push(u64::from_le_bytes(bytes[off..off + 8].try_into().map_err(|_| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_limb")
            })?));
        }
        assert_canonical_magnitude_limbs(count, &limbs)?;
        Self::from_limbs(limbs)
    }

    /// 解码 [`Self::wire_encode_magnitude`] 字节。
    pub(crate) fn wire_decode_magnitude(bytes: &[u8]) -> Result<Self> {
        Self::wire_decode_magnitude_budgeted(bytes, &crate::policy::execution_budget::ExecutionBudget::unlimited())
    }

    /// 从拼接的有理载荷中拆出首个幅度块。
    pub(crate) fn wire_take_magnitude(bytes: &[u8]) -> Result<(Self, &[u8])> {
        if bytes.len() < 4 {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_short"));
        }
        let count = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| {
            Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_count")
        })?) as usize;
        let total = 4usize
            .checked_add(count.checked_mul(8).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_overflow")
            })?)
            .ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "wire_magnitude_overflow")
            })?;
        if bytes.len() < total {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_magnitude_truncated"));
        }
        let mag = Self::wire_decode_magnitude(&bytes[..total])?;
        Ok((mag, &bytes[total..]))
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

impl FromStr for Natural {
    type Err = ();

    /// 十进制解析（仅数字，无符号）。
    fn from_str(digits: &str) -> std::result::Result<Self, Self::Err> {
        if digits.is_empty() {
            return Err(());
        }
        if !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(());
        }
        let mut n = Self::zero();
        for ch in digits.chars() {
            n = n.mul_u64(10).add_u64(u64::from(ch as u32 - u32::from(b'0')));
        }
        Ok(n)
    }
}

impl athena_gc::Trace for Natural {
    fn trace(&self, tracer: &mut dyn athena_gc::Tracer) {
        if let Some(ptr) = self.inner.heap_ptr() {
            tracer.mark_allocation(ptr.as_ptr().cast());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integer::Integer;
    use std::collections::HashSet;

    #[test]
    fn natural_sign_bit_is_semantic_dont_care() {
        let a = Natural::from_u64(42);
        let b = Natural::from_pair(a.clone().into_pair().with_negative(true));
        assert_eq!(a, b);
        assert_eq!(a.cmp(&b), Ordering::Equal);

        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn integer_interprets_sign_bit() {
        assert_ne!(Integer::from_u64(42), Integer::from_i64(-42));
    }

    #[test]
    fn try_add_owned_reuses_heap_buffer_when_capacity_allows() {
        use crate::policy::execution_budget::ExecutionBudget;
        use athena_gc::{GcHeap, HeapBudget};

        let heap = GcHeap::new_shared(HeapBudget::default());
        let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
        let a = Natural::from_limbs_with_capacity_in(&ctx, &[1, 2, 3, 4], 8).expect("a");
        let ptr_before = a.inner.heap_ptr();
        assert_eq!(a.inner.heap_capacity(), Some(8));
        let b = Natural::from_limbs_in(&ctx, vec![5, 6, 7]).expect("b");
        let expected = a.try_add(&b, &ctx).expect("ref");
        let sum = a.try_add_owned(&b, &ctx).expect("owned");
        assert_eq!(sum.as_limbs(), expected.as_limbs());
        assert_eq!(sum.inner.heap_ptr(), ptr_before, "must reuse stolen heap buffer");
    }

    #[test]
    fn try_add_owned_matches_try_add_without_spare_capacity() {
        use crate::policy::execution_budget::ExecutionBudget;
        use athena_gc::{GcHeap, HeapBudget};

        let heap = GcHeap::new_shared(HeapBudget::default());
        let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
        // capacity == len：无法容纳进位槽，回退 try_add。
        let a = Natural::from_limbs_in(&ctx, vec![u64::MAX, u64::MAX, u64::MAX]).expect("a");
        let b = Natural::from_u64(1);
        let expected = a.try_add(&b, &ctx).expect("ref");
        let sum = a.try_add_owned(&b, &ctx).expect("owned");
        assert_eq!(sum.as_limbs(), expected.as_limbs());
    }

    #[test]
    fn try_mul_u64_owned_reuses_heap_when_capacity_allows() {
        use crate::policy::execution_budget::ExecutionBudget;
        use athena_gc::{GcHeap, HeapBudget};

        let heap = GcHeap::new_shared(HeapBudget::default());
        let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
        let a = Natural::from_limbs_with_capacity_in(&ctx, &[3, 4, 5], 6).expect("a");
        let ptr_before = a.inner.heap_ptr();
        let expected = a.try_mul_u64(7, &ctx).expect("ref");
        let prod = a.try_mul_u64_owned(7, &ctx).expect("owned");
        assert_eq!(prod.as_limbs(), expected.as_limbs());
        assert_eq!(prod.inner.heap_ptr(), ptr_before);
    }

    #[test]
    fn try_add_owned_self_add_aliases_safely() {
        use crate::policy::execution_budget::ExecutionBudget;
        use athena_gc::{GcHeap, HeapBudget};

        let heap = GcHeap::new_shared(HeapBudget::default());
        let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
        let a = Natural::from_limbs_with_capacity_in(&ctx, &[9, 8, 7], 8).expect("a");
        let expected = a.try_add(&a, &ctx).expect("ref");
        let sum = a.clone().try_add_owned(&a, &ctx).expect("owned self");
        assert_eq!(sum.as_limbs(), expected.as_limbs());
    }

    #[test]
    fn try_sub_owned_reuses_heap_when_capacity_allows() {
        use crate::policy::execution_budget::ExecutionBudget;
        use athena_gc::{GcHeap, HeapBudget};

        let heap = GcHeap::new_shared(HeapBudget::default());
        let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
        let a = Natural::from_limbs_with_capacity_in(&ctx, &[10, 20, 30, 40], 6).expect("a");
        let ptr_before = a.inner.heap_ptr();
        let b = Natural::from_limbs_in(&ctx, vec![1, 2, 3]).expect("b");
        let expected = a.try_sub(&b, &ctx).expect("ref");
        let diff = a.try_sub_owned(&b, &ctx).expect("owned");
        assert_eq!(diff.as_limbs(), expected.as_limbs());
        assert_eq!(diff.inner.heap_ptr(), ptr_before);
    }

    #[test]
    fn try_mul_owned_reuses_heap_on_schoolbook_with_spare_capacity() {
        use crate::policy::execution_budget::ExecutionBudget;
        use athena_gc::{GcHeap, HeapBudget};

        let heap = GcHeap::new_shared(HeapBudget::default());
        let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
        // 3×3 schoolbook need = 6；capacity 8 → steal。
        let a = Natural::from_limbs_with_capacity_in(&ctx, &[2, 3, 4], 8).expect("a");
        let ptr_before = a.inner.heap_ptr();
        let b = Natural::from_limbs_in(&ctx, vec![5, 6, 7]).expect("b");
        let expected = a.try_mul(&b, &ctx).expect("ref");
        let prod = a.try_mul_owned(&b, &ctx).expect("owned");
        assert_eq!(prod.as_limbs(), expected.as_limbs());
        assert_eq!(prod.inner.heap_ptr(), ptr_before);
    }

    #[test]
    fn try_mul_owned_falls_back_without_spare_capacity() {
        use crate::policy::execution_budget::ExecutionBudget;
        use athena_gc::{GcHeap, HeapBudget};

        let heap = GcHeap::new_shared(HeapBudget::default());
        let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
        // capacity == 3 < need 6 → 回退 try_mul。
        let a = Natural::from_limbs_in(&ctx, vec![2, 3, 4]).expect("a");
        let b = Natural::from_limbs_in(&ctx, vec![5, 6, 7]).expect("b");
        let expected = a.try_mul(&b, &ctx).expect("ref");
        let prod = a.try_mul_owned(&b, &ctx).expect("owned");
        assert_eq!(prod.as_limbs(), expected.as_limbs());
    }
}
