//! [`Natural`] 公开算术入口（算法实现仍在本文件或委托 executor/kernel）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::Natural;
use crate::{
    kernel::limb as limb_kernel,
    policy::execution_budget::NumericContext,
    storage::{MagnitudePair, Mode},
};

impl Natural {
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
            return self
                .try_clone_in(&crate::policy::execution_budget::NumericContext::portable_default())
                .expect("portable default unbounded");
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
    /// 减法（要求 `self >= rhs`；默认上下文）。
    pub fn sub(&self, rhs: &Self) -> Self {
        self.try_sub(rhs, &NumericContext::portable_default()).expect("natural sub precondition or unbounded default")
    }

    /// 减法（`self >= rhs`；服从 `ctx` 预算）。
    pub fn try_sub(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        crate::dispatch::NumericExecutor::sub_natural(self, rhs, ctx)
    }

    /// 乘法（默认 [`NumericContext::portable_default`]）。
    pub fn mul(&self, rhs: &Self) -> Self {
        self.try_mul(rhs, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 乘法（服从 `ctx` 预算）。
    pub fn try_mul(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        crate::dispatch::NumericExecutor::mul_natural(self, rhs, ctx)
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
}
