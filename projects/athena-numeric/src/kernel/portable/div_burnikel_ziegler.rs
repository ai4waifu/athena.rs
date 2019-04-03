//! # Purpose
//! Burnikel–Ziegler block division: recurse on high half, then one Knuth step.
//!
//! # Mathematical model
//! Write $u = u_1 \beta^n + u_0$ with $n = |v|$. Divide $u_1$ by $v$ to get
//! $(q_1, r_1)$, form $u' = r_1 \beta^n + u_0$, then divide $u'$ by $v$ for
//! $(q_0, r)$. The full quotient is $q_1 \beta^n + q_0$.
//!
//! # Derivation
//! Follows the schoolbook identity for base-$\beta^n$ digits of the dividend
//! when the dividend has at least two divisor-sized blocks.
//!
//! # Algorithm steps
//! 1. If $|u| < 2|v|$, fall back to Knuth.
//! 2. Recurse on high block; assemble mid value; Knuth-divide mid; merge quotients.
//!
//! # Preconditions
//! - Multi-limb divisor; scratch shared with Knuth leaves.
//!
//! # Postconditions
//! - Same as Knuth: $u = q v + r$, $0 \le r < v$.
//!
//! # Complexity
//! Improves constants when $|u| \gg |v|$ by reducing Knuth digit loops on the high part.
//!
//! # Crossover
//! DIV_BZ_THRESHOLD and capability z_division in AlgorithmPlanner.
//!
//! # Failure modes
//! Same budget / div-zero paths as Knuth via shared helpers.
//!
//! # Tests
//! 	ests/exact/algorithms.rs (BZ vs Knuth capability gate).

use athena_types::Result;

use crate::{
    kernel::{LimbBuffer, ScratchWorkspace},
    policy::execution_budget::ExecutionBudget,
};

use super::{
    div_knuth::div_rem_knuth_into,
    primitive::{adc, effective_len},
};

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
