//! Burnikel–Ziegler division (block recursion; falls back to Knuth).

use athena_types::Result;

use crate::kernel::{LimbBuffer, ScratchWorkspace};
use crate::policy::execution_budget::ExecutionBudget;

use super::div_knuth::div_rem_knuth_into;
use super::primitive::{adc, effective_len};

/// Burnikel–Ziegler：大被除数按除数宽度切块递归；小情况回退 Knuth。
pub(super) fn div_rem_bz_into(
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
        return div_rem_knuth_into(u, v, q_out, r_out, scratch, budget);
    }
    let n = v_el;
    let u_lo = &u[..n.min(u_el)];
    let u_hi = if u_el > n { &u[n..u_el] } else { &[0u64][..] };

    let mut q_hi = LimbBuffer::zero();
    let mut r_hi = LimbBuffer::zero();
    div_rem_bz_into(u_hi, v, &mut q_hi, &mut r_hi, scratch, budget)?;

    let mut mid = LimbBuffer::zero();
    {
        let need = r_hi.as_canonical().len() + n + 1;
        budget.check_limbs(need)?;
        let storage = mid.storage_mut(need, budget)?;
        storage.fill(0);
        let rh = r_hi.as_canonical();
        storage[n..n + rh.len()].copy_from_slice(rh);
        let lo_n = effective_len(u_lo);
        let mut carry = 0u64;
        for i in 0..lo_n {
            let (sum, c) = adc(storage[i], u_lo[i], carry);
            storage[i] = sum;
            carry = c;
        }
        let mut i = lo_n;
        while carry > 0 && i < storage.len() {
            let (sum, c) = adc(storage[i], 0, carry);
            storage[i] = sum;
            carry = c;
            i += 1;
        }
        mid.trim_canonical();
    }
    let mut q_lo = LimbBuffer::zero();
    div_rem_knuth_into(mid.as_canonical(), v, &mut q_lo, r_out, scratch, budget)?;

    let qh = q_hi.as_canonical();
    let ql = q_lo.as_canonical();
    let need = qh.len() + n + ql.len() + 1;
    budget.check_limbs(need)?;
    let storage = q_out.storage_mut(need, budget)?;
    storage.fill(0);
    storage[..ql.len()].copy_from_slice(ql);
    let mut carry = 0u64;
    for i in 0..qh.len() {
        let idx = i + n;
        let (sum, c) = adc(storage[idx], qh[i], carry);
        storage[idx] = sum;
        carry = c;
    }
    let mut idx = qh.len() + n;
    while carry > 0 && idx < storage.len() {
        let (sum, c) = adc(storage[idx], 0, carry);
        storage[idx] = sum;
        carry = c;
        idx += 1;
    }
    q_out.trim_canonical();
    Ok(())
}
