//! Portable `LimbKernel` binding (operation entry points).
//!
//! Strategy is chosen by `AlgorithmPlanner` and passed in. This module only executes.

use athena_types::Result;
use std::cmp::Ordering;

use crate::{
    algorithm::{DivStrategy, MulStrategy, karatsuba_scratch_limbs, toom3_scratch_limbs},
    kernel::{LimbBuffer, ScratchWorkspace, kernel_err},
    policy::execution_budget::ExecutionBudget,
};

use super::{
    div_burnikel_ziegler::div_rem_bz_into,
    div_knuth::div_rem_knuth_into,
    div_single::div_rem_1_into,
    mul_karatsuba::mul_rec,
    mul_schoolbook::{mul_1_into_slice, mul_schoolbook_into_soft, sqr_schoolbook_into},
    mul_toom3::toom3_mul_rec,
    primitive::{adc, cmp_slice, effective_len, is_zero, sbb},
};

/// 私有 limb 执行合同（输出缓冲 + scratch + 预算）。
pub(crate) trait LimbKernel {
    fn add_into(a: &[u64], b: &[u64], out: &mut LimbBuffer, scratch: &mut ScratchWorkspace, budget: &ExecutionBudget) -> Result<()>;

    fn sub_into(a: &[u64], b: &[u64], out: &mut LimbBuffer, scratch: &mut ScratchWorkspace, budget: &ExecutionBudget) -> Result<()>;

    fn mul_into(
        a: &[u64],
        b: &[u64],
        strategy: MulStrategy,
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()>;

    fn mul_1_into(a: &[u64], limb: u64, out: &mut LimbBuffer, scratch: &mut ScratchWorkspace, budget: &ExecutionBudget) -> Result<()>;

    fn sqr_into(a: &[u64], strategy: MulStrategy, out: &mut LimbBuffer, scratch: &mut ScratchWorkspace, budget: &ExecutionBudget)
    -> Result<()>;

    fn div_rem_into(
        u: &[u64],
        v: &[u64],
        strategy: DivStrategy,
        q_out: &mut LimbBuffer,
        r_out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()>;
}

pub(crate) struct PortableLimbKernel;

impl LimbKernel for PortableLimbKernel {
    fn add_into(a: &[u64], b: &[u64], out: &mut LimbBuffer, _scratch: &mut ScratchWorkspace, budget: &ExecutionBudget) -> Result<()> {
        let la = effective_len(a);
        let lb = effective_len(b);
        budget.check_add(la, lb)?;
        let n = la.max(lb);
        let storage = out.storage_mut(n + 1, budget)?;
        storage.fill(0);
        let mut carry = 0u64;
        for i in 0..n {
            let (sum, c) = adc(*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0), carry);
            storage[i] = sum;
            carry = c;
        }
        storage[n] = carry;
        out.trim_canonical();
        Ok(())
    }

    fn sub_into(a: &[u64], b: &[u64], out: &mut LimbBuffer, _scratch: &mut ScratchWorkspace, budget: &ExecutionBudget) -> Result<()> {
        if cmp_slice(a, b) == Ordering::Less {
            return Err(kernel_err("sub_underflow"));
        }
        let n = effective_len(a);
        budget.check_limbs(n)?;
        let storage = out.storage_mut(n, budget)?;
        storage.fill(0);
        let mut borrow = 0u64;
        for i in 0..n {
            let (diff, b_out) = sbb(*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0), borrow);
            storage[i] = diff;
            borrow = b_out;
        }
        out.trim_canonical();
        Ok(())
    }

    fn mul_into(
        a: &[u64],
        b: &[u64],
        strategy: MulStrategy,
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        if matches!(strategy, MulStrategy::Zero) || is_zero(a) || is_zero(b) {
            return out.set_zero(budget);
        }
        let la = effective_len(a);
        let lb = effective_len(b);
        budget.check_mul(la, lb)?;
        match strategy {
            MulStrategy::Zero => out.set_zero(budget),
            MulStrategy::Schoolbook => {
                let storage = out.storage_mut(la + lb, budget)?;
                storage.fill(0);
                mul_schoolbook_into_soft(a, b, storage);
                out.trim_canonical();
                Ok(())
            }
            MulStrategy::Karatsuba => {
                let scratch_need = karatsuba_scratch_limbs(la.max(lb));
                budget.check_limbs(scratch_need.max(la + lb))?;
                scratch.ensure(scratch_need, budget)?;
                let out_len = la + lb;
                let storage = out.storage_mut(out_len, budget)?;
                storage.fill(0);
                let scratch_slice = scratch.as_mut_slice();
                mul_rec(a, b, storage, scratch_slice);
                out.trim_canonical();
                Ok(())
            }
            MulStrategy::Toom3 => {
                let scratch_need = toom3_scratch_limbs(la.max(lb));
                budget.check_limbs(scratch_need.max(la + lb))?;
                scratch.ensure(scratch_need, budget)?;
                let out_len = la + lb;
                let storage = out.storage_mut(out_len, budget)?;
                storage.fill(0);
                let scratch_slice = scratch.as_mut_slice();
                toom3_mul_rec(a, b, storage, scratch_slice);
                out.trim_canonical();
                Ok(())
            }
        }
    }

    fn mul_1_into(a: &[u64], limb: u64, out: &mut LimbBuffer, _scratch: &mut ScratchWorkspace, budget: &ExecutionBudget) -> Result<()> {
        if limb == 0 || is_zero(a) {
            return out.set_zero(budget);
        }
        let la = effective_len(a);
        budget.check_mul(la, 1)?;
        let storage = out.storage_mut(la + 1, budget)?;
        mul_1_into_slice(a, limb, storage);
        out.trim_canonical();
        Ok(())
    }

    fn sqr_into(
        a: &[u64],
        strategy: MulStrategy,
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        if matches!(strategy, MulStrategy::Zero) || is_zero(a) {
            return out.set_zero(budget);
        }
        let la = effective_len(a);
        budget.check_mul(la, la)?;
        match strategy {
            MulStrategy::Zero => out.set_zero(budget),
            MulStrategy::Schoolbook => {
                let storage = out.storage_mut(2 * la, budget)?;
                sqr_schoolbook_into(a, storage);
                out.trim_canonical();
                Ok(())
            }
            MulStrategy::Karatsuba | MulStrategy::Toom3 => Self::mul_into(a, a, strategy, out, scratch, budget),
        }
    }

    fn div_rem_into(
        u: &[u64],
        v: &[u64],
        strategy: DivStrategy,
        q_out: &mut LimbBuffer,
        r_out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        let u_el = effective_len(u);
        let v_el = effective_len(v);
        if v_el == 1 && v.get(0).copied().unwrap_or(0) == 0 {
            return Err(kernel_err("div_zero"));
        }
        if is_zero(v) {
            return Err(kernel_err("div_zero"));
        }
        budget.check_div(u_el, v_el)?;

        if is_zero(u) || cmp_slice(u, v) == Ordering::Less {
            q_out.set_zero(budget)?;
            r_out.copy_canonical(&u[..u_el.max(1)], budget)?;
            return Ok(());
        }
        if v_el == 1 {
            return div_rem_1_into(u, v[0], q_out, r_out, budget);
        }
        match strategy {
            DivStrategy::Knuth => div_rem_knuth_into(u, v, q_out, r_out, scratch, budget),
            DivStrategy::BurnikelZiegler => div_rem_bz_into(u, v, q_out, r_out, scratch, budget),
        }
    }
}
