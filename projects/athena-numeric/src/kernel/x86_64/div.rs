//! x86_64 Knuth / Burnikel–Ziegler：`mulx` addmul·submul 叶 + ADX 进位链。
//!
//! Portable 侧 Knuth 走 soft addmul/submul 作 parity 基线；本模块是
//! `KernelTable::div_rem_into` 的 ISA 执行体（非整表委派 portable）。

use core::cmp::Ordering;

use athena_types::Result;

use crate::{
    algorithm::DivStrategy,
    kernel::{
        LimbBuffer, ScratchWorkspace, kernel_err,
        portable::{self, LimbKernel, PortableLimbKernel, cmp_slice, effective_len, is_zero},
    },
    policy::execution_budget::ExecutionBudget,
};

use super::{adc_chain, addmul_1_inplace_isa, submul_1_inplace_isa};

/// `KernelTable` 除法入口：单 limb / Knuth / BZ 调度。
pub(super) fn div_rem_into_isa(
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
        // 单 limb 与 portable 同合同；宽路径才吃 ISA addmul/submul。
        return <PortableLimbKernel as LimbKernel>::div_rem_into(u, v, strategy, q_out, r_out, scratch, budget);
    }
    match strategy {
        DivStrategy::Knuth => div_rem_knuth_isa(u, v, q_out, r_out, scratch, budget),
        DivStrategy::BurnikelZiegler => div_rem_bz_isa(u, v, q_out, r_out, scratch, budget),
    }
}

fn div_rem_knuth_isa(
    u: &[u64],
    v: &[u64],
    q_out: &mut LimbBuffer,
    r_out: &mut LimbBuffer,
    scratch: &mut ScratchWorkspace,
    budget: &ExecutionBudget,
) -> Result<()> {
    let n = effective_len(v);
    debug_assert!(n >= 2);
    let u_el = effective_len(u);
    let m = u_el.saturating_sub(n);
    let shift = v[n - 1].leading_zeros();

    let need = portable::div_knuth::div_scratch_limbs(u_el, n);
    scratch.ensure(need, budget)?;
    let arena = scratch.as_mut_slice();
    let u_cap = m + n + 1;
    let (u_work, rest) = arena.split_at_mut(u_cap);
    let (v_work, rest) = rest.split_at_mut(n);
    let (q_work, rest) = rest.split_at_mut(m + 1);
    let (r_work, _) = rest.split_at_mut(n);

    u_work.fill(0);
    v_work.fill(0);
    q_work.fill(0);
    r_work.fill(0);

    if shift > 0 {
        let _ = portable::div_knuth::shl_into(u, shift, u_work);
        let _ = portable::div_knuth::shl_into(v, shift, v_work);
    }
    else {
        u_work[..u_el].copy_from_slice(&u[..u_el]);
        v_work.copy_from_slice(&v[..n]);
    }

    let v_n1 = v_work[n - 1];
    let v_n2 = v_work[n - 2];

    for j in (0..=m).rev() {
        let u_jn = u_work[j + n];
        let u_jn1 = u_work[j + n - 1];
        let top = (u128::from(u_jn) << 64) | u128::from(u_jn1);
        let mut qhat = if u_jn >= v_n1 { u64::MAX } else { (top / u128::from(v_n1)) as u64 };
        let mut rhat = top - u128::from(qhat) * u128::from(v_n1);

        while u128::from(qhat) >= (1u128 << 64) || (u128::from(qhat) * u128::from(v_n2) > (rhat << 64) + u128::from(u_work[j + n - 2])) {
            qhat = qhat.wrapping_sub(1);
            rhat += u128::from(v_n1);
            if rhat >= (1u128 << 64) {
                break;
            }
        }

        let borrow = submul_1_inplace_isa(&mut u_work[j..j + n + 1], v_work, qhat);
        if borrow {
            qhat = qhat.wrapping_sub(1);
            let _ = addmul_1_inplace_isa(&mut u_work[j..j + n + 1], v_work, 1);
        }
        q_work[j] = qhat;
    }

    let qn = effective_len(q_work).max(1);
    q_out.copy_canonical(&q_work[..qn], budget)?;

    if shift > 0 {
        let r_len = portable::div_knuth::shr_into(&u_work[..n], shift, r_work);
        r_out.copy_canonical(&r_work[..r_len], budget)?;
    }
    else {
        let rn = effective_len(&u_work[..n]).max(1);
        r_out.copy_canonical(&u_work[..rn], budget)?;
    }
    Ok(())
}

fn div_rem_bz_isa(
    u: &[u64],
    v: &[u64],
    q_out: &mut LimbBuffer,
    r_out: &mut LimbBuffer,
    scratch: &mut ScratchWorkspace,
    budget: &ExecutionBudget,
) -> Result<()> {
    let u_el = effective_len(u);
    let v_el = effective_len(v);
    if u_el < 2 * v_el {
        return div_rem_knuth_isa(u, v, q_out, r_out, scratch, budget);
    }
    let n = v_el;
    let u_lo = &u[..n.min(u_el)];
    let u_hi = if u_el > n { &u[n..u_el] } else { &[0u64][..] };

    let mut q_hi = LimbBuffer::zero();
    let mut r_hi = LimbBuffer::zero();
    div_rem_bz_isa(u_hi, v, &mut q_hi, &mut r_hi, scratch, budget)?;

    let mut mid = LimbBuffer::zero();
    {
        let need = r_hi.as_canonical().len() + n + 1;
        budget.check_limbs(need)?;
        let storage = mid.storage_mut(need, budget)?;
        storage.fill(0);
        let rh = r_hi.as_canonical();
        storage[n..n + rh.len()].copy_from_slice(rh);
        let lo_n = effective_len(u_lo);
        let mut carry = 0u8;
        for i in 0..lo_n {
            let (sum, c) = adc_chain(carry, storage[i], u_lo[i]);
            storage[i] = sum;
            carry = c;
        }
        let mut i = lo_n;
        while carry > 0 && i < storage.len() {
            let (sum, c) = adc_chain(carry, storage[i], 0);
            storage[i] = sum;
            carry = c;
            i += 1;
        }
        mid.trim_canonical();
    }
    let mut q_lo = LimbBuffer::zero();
    div_rem_knuth_isa(mid.as_canonical(), v, &mut q_lo, r_out, scratch, budget)?;

    let qh = q_hi.as_canonical();
    let ql = q_lo.as_canonical();
    let need = qh.len() + n + ql.len() + 1;
    budget.check_limbs(need)?;
    let storage = q_out.storage_mut(need, budget)?;
    storage.fill(0);
    storage[..ql.len()].copy_from_slice(ql);
    let mut carry = 0u8;
    for i in 0..qh.len() {
        let idx = i + n;
        let (sum, c) = adc_chain(carry, storage[idx], qh[i]);
        storage[idx] = sum;
        carry = c;
    }
    let mut idx = qh.len() + n;
    while carry > 0 && idx < storage.len() {
        let (sum, c) = adc_chain(carry, storage[idx], 0);
        storage[idx] = sum;
        carry = c;
        idx += 1;
    }
    q_out.trim_canonical();
    Ok(())
}
