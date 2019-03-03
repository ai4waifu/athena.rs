//! 宽度分派 executor：读 meta / limb 宽度一次，再调已绑定 `KernelTable`。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    kernel::limb as limb_kernel,
    policy::NumericContext,
    storage::{LimbWidth, Mode},
    value::natural::Natural,
};

/// 数值执行器（持有 planner 视图；kernel 来自 context）。
#[derive(Debug, Clone, Copy, Default)]
pub struct NumericExecutor;

impl NumericExecutor {
    /// 借用 limb 切片加法；结果发布到 `ctx` heap。
    pub fn add_limbs(lhs: &[u64], rhs: &[u64], ctx: &NumericContext) -> Result<Natural> {
        ctx.check_entry()?;
        match (LimbWidth::classify(lhs), LimbWidth::classify(rhs)) {
            (LimbWidth::Zero, _) => Natural::from_limb_slice_in(ctx, rhs),
            (_, LimbWidth::Zero) => Natural::from_limb_slice_in(ctx, lhs),
            (LimbWidth::Limb1(a), LimbWidth::Limb1(b)) => {
                let kernels = ctx.kernels();
                ctx.budget().check_limbs(2)?;
                let (lo, carry) = kernels.add_1(ctx.kernel_token(), a, b);
                Ok(if carry == 0 { Natural::from_u64(lo) } else { Natural::from_limb2([lo, 1]) })
            }
            (LimbWidth::Limb1(a), LimbWidth::Limb2(b)) => {
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_1_2(a, b);
                Natural::from_limb_slice_in(ctx, &limbs[..len])
            }
            (LimbWidth::Limb2(a), LimbWidth::Limb1(b)) => {
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_1_2(b, a);
                Natural::from_limb_slice_in(ctx, &limbs[..len])
            }
            (LimbWidth::Limb2(a), LimbWidth::Limb2(b)) => {
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_2(a, b);
                Natural::from_limb_slice_in(ctx, &limbs[..len])
            }
            (la, lb) => {
                let a = width_limbs(lhs, la);
                let b = width_limbs(rhs, lb);
                let kernels = ctx.kernels();
                Natural::publish_with_kernel(ctx, |out, scratch, budget| {
                    kernels.add_into(ctx.kernel_token(), a, b, out, scratch, budget)
                })
            }
        }
    }

    /// 借用 limb 切片乘法；结果发布到 `ctx` heap。
    pub fn mul_limbs(lhs: &[u64], rhs: &[u64], ctx: &NumericContext) -> Result<Natural> {
        ctx.check_entry()?;
        match (LimbWidth::classify(lhs), LimbWidth::classify(rhs)) {
            (LimbWidth::Zero, _) | (_, LimbWidth::Zero) => Ok(Natural::zero()),
            (LimbWidth::Limb1(a), LimbWidth::Limb1(b)) => {
                let kernels = ctx.kernels();
                ctx.budget().check_limbs(2)?;
                Ok(Natural::from_u128_mag(kernels.mul_1x1(ctx.kernel_token(), a, b)))
            }
            (LimbWidth::Limb1(a), LimbWidth::Limb2(b)) => {
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::mul_2x1(b, a);
                Natural::from_limb_slice_in(ctx, &limbs[..len])
            }
            (LimbWidth::Limb2(a), LimbWidth::Limb1(b)) => {
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::mul_2x1(a, b);
                Natural::from_limb_slice_in(ctx, &limbs[..len])
            }
            (LimbWidth::Limb2(a), LimbWidth::Limb2(b)) => {
                ctx.budget().check_limbs(4)?;
                let (limbs, len) = limb_kernel::mul_2(a, b);
                Natural::from_limb_slice_in(ctx, &limbs[..len])
            }
            (la, lb) => {
                let a = width_limbs(lhs, la);
                let b = width_limbs(rhs, lb);
                let plan = ctx.planner().plan_mul(a.len(), b.len());
                let kernels = ctx.kernels();
                Natural::publish_with_kernel(ctx, |out, scratch, budget| {
                    kernels.mul_into(ctx.kernel_token(), a, b, plan, out, scratch, budget)
                })
            }
        }
    }

    /// 借用 limb 切片减法（`lhs >= rhs`）；结果发布到 `ctx` heap。
    pub fn sub_limbs(lhs: &[u64], rhs: &[u64], ctx: &NumericContext) -> Result<Natural> {
        ctx.check_entry()?;
        if limb_kernel::cmp_slice(lhs, rhs).is_lt() {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "numeric")
                .detail("operation", "natural_sub")
                .detail("reason", "underflow"));
        }
        match (LimbWidth::classify(lhs), LimbWidth::classify(rhs)) {
            (_, LimbWidth::Zero) => Natural::from_limb_slice_in(ctx, lhs),
            (LimbWidth::Limb1(a), LimbWidth::Limb1(b)) => {
                ctx.budget().check_limbs(1)?;
                Ok(Natural::from_u64(limb_kernel::sub_1(a, b)))
            }
            (LimbWidth::Limb2(a), LimbWidth::Limb1(b)) => {
                ctx.budget().check_limbs(2)?;
                let (limbs, len) = limb_kernel::sub_2_1(a, b);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            (LimbWidth::Limb2(a), LimbWidth::Limb2(b)) => {
                ctx.budget().check_limbs(2)?;
                let (limbs, len) = limb_kernel::sub_2(a, b);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            (la, lb) => {
                let a = width_limbs(lhs, la);
                let b = width_limbs(rhs, lb);
                let kernels = ctx.kernels();
                Natural::publish_with_kernel(ctx, |out, scratch, budget| {
                    kernels.sub_into(ctx.kernel_token(), a, b, out, scratch, budget)
                })
            }
        }
    }

    /// `Natural` 加法（宽度分派在此，不在 value 内嵌 ISA 分支）。
    pub fn add_natural(lhs: &Natural, rhs: &Natural, ctx: &NumericContext) -> Result<Natural> {
        ctx.check_entry()?;
        if lhs.is_zero() {
            return Natural::from_limb_slice_in(ctx, rhs.as_limbs());
        }
        if rhs.is_zero() {
            return Natural::from_limb_slice_in(ctx, lhs.as_limbs());
        }
        match (lhs.mode(), rhs.mode()) {
            (Mode::Limb1, Mode::Limb1) => {
                let kernels = ctx.kernels();
                let a = lhs.limb1().expect("Limb1");
                let b = rhs.limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                let (lo, carry) = kernels.add_1(ctx.kernel_token(), a, b);
                Ok(if carry == 0 { Natural::from_u64(lo) } else { Natural::from_limb2([lo, 1]) })
            }
            (Mode::Limb1, Mode::Limb2) => {
                let a = lhs.limb1().expect("Limb1");
                let b = rhs.limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_1_2(a, b);
                Natural::from_limb_slice_in(ctx, &limbs[..len])
            }
            (Mode::Limb2, Mode::Limb1) => {
                let a = lhs.limb2().expect("Limb2");
                let b = rhs.limb1().expect("Limb1");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_1_2(b, a);
                Natural::from_limb_slice_in(ctx, &limbs[..len])
            }
            (Mode::Limb2, Mode::Limb2) => {
                let a = lhs.limb2().expect("Limb2");
                let b = rhs.limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_2(a, b);
                Natural::from_limb_slice_in(ctx, &limbs[..len])
            }
            _ => {
                let kernels = ctx.kernels();
                Natural::publish_with_kernel(ctx, |out, scratch, budget| {
                    kernels.add_into(ctx.kernel_token(), lhs.as_limbs(), rhs.as_limbs(), out, scratch, budget)
                })
            }
        }
    }

    /// `Natural` 乘法。
    pub fn mul_natural(lhs: &Natural, rhs: &Natural, ctx: &NumericContext) -> Result<Natural> {
        Self::mul_limbs(lhs.as_limbs(), rhs.as_limbs(), ctx)
    }

    /// `Natural` 减法（`lhs >= rhs`）。
    pub fn sub_natural(lhs: &Natural, rhs: &Natural, ctx: &NumericContext) -> Result<Natural> {
        Self::sub_limbs(lhs.as_limbs(), rhs.as_limbs(), ctx)
    }

    /// 借用 limb 除法与余数；结果发布到 `ctx` heap。
    pub fn div_rem_limbs(lhs: &[u64], rhs: &[u64], ctx: &NumericContext) -> Result<(Natural, Natural)> {
        ctx.check_entry()?;
        if limb_kernel::is_zero(rhs) || rhs.is_empty() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "natural_div_rem"));
        }
        if limb_kernel::is_zero(lhs) || lhs.is_empty() {
            return Ok((Natural::zero(), Natural::zero()));
        }
        match (LimbWidth::classify(lhs), LimbWidth::classify(rhs)) {
            (LimbWidth::Limb1(a), LimbWidth::Limb1(b)) => {
                ctx.budget().check_limbs(1)?;
                let (q, r) = limb_kernel::div_rem_1(a, b);
                Ok((Natural::from_u64(q), Natural::from_u64(r)))
            }
            (LimbWidth::Limb1(_) | LimbWidth::Limb2(_), LimbWidth::Limb1(_) | LimbWidth::Limb2(_)) => {
                let numer = match LimbWidth::classify(lhs) {
                    LimbWidth::Limb1(a) => a as u128,
                    LimbWidth::Limb2(a) => a[0] as u128 | ((a[1] as u128) << 64),
                    _ => unreachable!(),
                };
                let denom = match LimbWidth::classify(rhs) {
                    LimbWidth::Limb1(b) => b as u128,
                    LimbWidth::Limb2(b) => b[0] as u128 | ((b[1] as u128) << 64),
                    _ => unreachable!(),
                };
                ctx.budget().check_limbs(2)?;
                let (q, r) = limb_kernel::div_rem_u128(numer, denom);
                Ok((Natural::from_u128_mag(q), Natural::from_u128_mag(r)))
            }
            _ => {
                let a_len = limb_kernel::effective_len(lhs);
                let b_len = limb_kernel::effective_len(rhs);
                let mut q = crate::kernel::LimbBuffer::zero();
                let mut r = crate::kernel::LimbBuffer::zero();
                let plan = ctx.planner().plan_div(a_len, b_len);
                ctx.with_scratch_frame(|scratch, budget| {
                    ctx.kernels().div_rem_into(
                        ctx.kernel_token(),
                        &lhs[..a_len],
                        &rhs[..b_len],
                        plan,
                        &mut q,
                        &mut r,
                        scratch,
                        budget,
                    )
                })?;
                Ok((
                    Natural::from_limb_slice_in(ctx, q.as_canonical())?,
                    Natural::from_limb_slice_in(ctx, r.as_canonical())?,
                ))
            }
        }
    }

    /// `Natural` 除法与余数。
    pub fn div_rem_natural(lhs: &Natural, rhs: &Natural, ctx: &NumericContext) -> Result<(Natural, Natural)> {
        Self::div_rem_limbs(lhs.as_limbs(), rhs.as_limbs(), ctx)
    }

    /// 借用 limb GCD；结果发布到 `ctx` heap（策略由 planner 唯一选择）。
    pub fn gcd_limbs(lhs: &[u64], rhs: &[u64], ctx: &NumericContext) -> Result<Natural> {
        ctx.check_entry()?;
        let la = if lhs.is_empty() || limb_kernel::is_zero(lhs) {
            0
        }
        else {
            limb_kernel::effective_len(lhs)
        };
        let lb = if rhs.is_empty() || limb_kernel::is_zero(rhs) {
            0
        }
        else {
            limb_kernel::effective_len(rhs)
        };
        if la == 0 && lb == 0 {
            return Ok(Natural::zero());
        }
        let bound = la.min(lb).max(1);
        ctx.budget().check_limbs(bound)?;
        let a = if la == 0 { vec![0] } else { lhs[..la].to_vec() };
        let b = if lb == 0 { vec![0] } else { rhs[..lb].to_vec() };
        let plan = ctx.planner().plan_gcd(la, lb);
        let limbs = match plan {
            crate::algorithm::GcdStrategy::Binary => limb_kernel::binary_gcd(a, b),
            crate::algorithm::GcdStrategy::Lehmer => limb_kernel::gcd(a, b),
            crate::algorithm::GcdStrategy::HalfGcd => limb_kernel::half_gcd(a, b),
        };
        Natural::from_limbs_in(ctx, limbs)
    }

    /// `Natural` GCD。
    pub fn gcd_natural(lhs: &Natural, rhs: &Natural, ctx: &NumericContext) -> Result<Natural> {
        Self::gcd_limbs(lhs.as_limbs(), rhs.as_limbs(), ctx)
    }
}

#[inline]
fn width_limbs<'a>(src: &'a [u64], width: LimbWidth<'a>) -> &'a [u64] {
    match width {
        LimbWidth::Zero => &[],
        LimbWidth::Limb1(_) => &src[..1],
        LimbWidth::Limb2(_) => &src[..2],
        LimbWidth::Wide(w) => w,
    }
}
