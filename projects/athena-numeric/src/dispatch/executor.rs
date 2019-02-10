//! 宽度分派 executor：读 meta 一次，再调已绑定 `KernelTable`。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    algorithm::AlgorithmPlanner,
    policy::NumericContext,
    storage::Mode,
    value::natural::Natural,
};

/// 数值执行器（持有 planner 视图；kernel 来自 context）。
#[derive(Debug, Clone, Copy, Default)]
pub struct NumericExecutor;

impl NumericExecutor {
    /// `Natural` 加法（宽度分派在此，不在 value 内嵌 ISA 分支）。
    pub fn add_natural(lhs: &Natural, rhs: &Natural, ctx: &NumericContext) -> Result<Natural> {
        ctx.check_entry()?;
        if lhs.is_zero() {
            return Ok(rhs.clone());
        }
        if rhs.is_zero() {
            return Ok(lhs.clone());
        }
        let kernels = ctx.kernels();
        match (lhs.mode(), rhs.mode()) {
            (Mode::Limb1, Mode::Limb1) => {
                let a = lhs.limb1().expect("Limb1");
                let b = rhs.limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                let (lo, carry) = kernels.add_1(ctx.kernel_token(), a, b);
                Ok(if carry == 0 {
                    Natural::from_u64(lo)
                }
                else {
                    Natural::from_limb2([lo, 1])
                })
            }
            (Mode::Limb1, Mode::Limb2) => {
                let a = lhs.limb1().expect("Limb1");
                let b = rhs.limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = crate::kernel::limb::add_1_2(a, b);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            (Mode::Limb2, Mode::Limb1) => {
                let a = lhs.limb2().expect("Limb2");
                let b = rhs.limb1().expect("Limb1");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = crate::kernel::limb::add_1_2(b, a);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            (Mode::Limb2, Mode::Limb2) => {
                let a = lhs.limb2().expect("Limb2");
                let b = rhs.limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = crate::kernel::limb::add_2(a, b);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            _ => Natural::publish_with_kernel(ctx, |out, scratch, budget| {
                kernels.add_into(ctx.kernel_token(), lhs.as_limbs(), rhs.as_limbs(), out, scratch, budget)
            }),
        }
    }

    /// `Natural` 乘法。
    pub fn mul_natural(lhs: &Natural, rhs: &Natural, ctx: &NumericContext) -> Result<Natural> {
        ctx.check_entry()?;
        if lhs.is_zero() || rhs.is_zero() {
            return Ok(Natural::zero());
        }
        let kernels = ctx.kernels();
        let _plan = AlgorithmPlanner::new(ctx.capabilities()).plan_mul(lhs.limb_len(), rhs.limb_len());
        match (lhs.mode(), rhs.mode()) {
            (Mode::Limb1, Mode::Limb1) => {
                let a = lhs.limb1().expect("Limb1");
                let b = rhs.limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                Ok(Natural::from_u128_mag(kernels.mul_1x1(ctx.kernel_token(), a, b)))
            }
            (Mode::Limb1, Mode::Limb2) => {
                let a = lhs.limb1().expect("Limb1");
                let b = rhs.limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = crate::kernel::limb::mul_2x1(b, a);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            (Mode::Limb2, Mode::Limb1) => {
                let a = lhs.limb2().expect("Limb2");
                let b = rhs.limb1().expect("Limb1");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = crate::kernel::limb::mul_2x1(a, b);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            (Mode::Limb2, Mode::Limb2) => {
                let a = lhs.limb2().expect("Limb2");
                let b = rhs.limb2().expect("Limb2");
                ctx.budget().check_limbs(4)?;
                let (limbs, len) = crate::kernel::limb::mul_2(a, b);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            _ => Natural::publish_with_kernel(ctx, |out, scratch, budget| {
                kernels.mul_into(ctx.kernel_token(), lhs.as_limbs(), rhs.as_limbs(), out, scratch, budget)
            }),
        }
    }

    /// `Natural` 减法（`lhs >= rhs`）。
    pub fn sub_natural(lhs: &Natural, rhs: &Natural, ctx: &NumericContext) -> Result<Natural> {
        ctx.check_entry()?;
        if lhs < rhs {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "numeric")
                .detail("operation", "natural_sub")
                .detail("reason", "underflow"));
        }
        let kernels = ctx.kernels();
        match (lhs.mode(), rhs.mode()) {
            (Mode::Limb1, Mode::Limb1) => {
                let a = lhs.limb1().expect("Limb1");
                let b = rhs.limb1().expect("Limb1");
                ctx.budget().check_limbs(1)?;
                Ok(Natural::from_u64(crate::kernel::limb::sub_1(a, b)))
            }
            (Mode::Limb2, Mode::Limb1) => {
                let a = lhs.limb2().expect("Limb2");
                let b = rhs.limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                let (limbs, len) = crate::kernel::limb::sub_2_1(a, b);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            (Mode::Limb2, Mode::Limb2) => {
                let a = lhs.limb2().expect("Limb2");
                let b = rhs.limb2().expect("Limb2");
                ctx.budget().check_limbs(2)?;
                let (limbs, len) = crate::kernel::limb::sub_2(a, b);
                Ok(Natural::from_fixed_limbs(&limbs[..len]))
            }
            _ => Natural::publish_with_kernel(ctx, |out, scratch, budget| {
                kernels.sub_into(ctx.kernel_token(), lhs.as_limbs(), rhs.as_limbs(), out, scratch, budget)
            }),
        }
    }
}
