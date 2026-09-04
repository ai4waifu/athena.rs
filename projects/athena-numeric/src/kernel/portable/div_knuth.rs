//! # Purpose
//! Knuth Algorithm D: normalized multi-limb division with quotient correction.
//!
//! # Mathematical model
//! For $u = q v + r$ with $0 \le r < v$, after left-shifting so $v$ has top
//! bit 1, each quotient digit estimated from the top two limbs of the working
//! dividend differs from the true digit by a small, correctable amount.
//!
//! # Derivation
//! Normalization maximizes the leading divisor limb. The estimate
//! $\hat q = \lfloor u_{j+n:j+n-1} / v_{n-1} \rfloor$ (capped) is refined by the
//! $v_{n-2}$ test, then by multiply-subtract; a borrow forces add-back.
//!
//! # Algorithm steps
//! 1. Compute shift = leading zeros of top divisor limb; shift $u,v$ into scratch.
//! 2. For $j = m..0$: estimate $\hat q$, correct, submul, optional add-back.
//! 3. Write quotient; right-shift remainder by the normalization amount.
//!
//! # Preconditions
//! - effective_len(v) >= 2 (single-limb uses div_single).
//! - Scratch capacity div_scratch_limbs; budget checked by caller/glue.
//!
//! # Postconditions
//! - $u = q v + r$, $0 \le r < v$ (canonical limbs).
//!
//! # Complexity
//! $\Theta((m+1) n)$ digit steps for $m+1$ quotient limbs and $n$-limb divisor.
//!
//! # Crossover
//! Planner default for multi-limb division; BZ only when dividend much wider.
//!
//! # Failure modes
//! Division by zero rejected in glue. Budget / capacity errors returned as Result.
//!
//! # Tests
//! tests/exact/algorithms.rs, natural div_rem identity suites.

use athena_types::Result;

use crate::{
    kernel::{LimbBuffer, ScratchWorkspace},
    policy::execution_budget::ExecutionBudget,
};

use super::{
    mul_schoolbook::{addmul_1_inplace_soft, submul_1_inplace_soft},
    primitive::{effective_len, is_zero},
    slice_ops::trim_slice_len,
};

/// Knuth 除法 scratch：归一化 u、v、商，以及可选余数右移缓冲。
pub(crate) fn div_scratch_limbs(u_limbs: usize, v_limbs: usize) -> usize {
    let m = u_limbs.saturating_sub(v_limbs);
    (m + v_limbs + 1) + v_limbs + (m + 1) + v_limbs
}

pub(crate) fn shl_into(v: &[u64], bits: u32, out: &mut [u64]) -> usize {
    let el = effective_len(v);
    out.fill(0);
    if bits == 0 || is_zero(v) {
        out[..el].copy_from_slice(&v[..el]);
        return el;
    }
    if bits >= 64 {
        // only used with bits < 64 in Knuth normalize
        debug_assert!(bits < 64);
    }
    let mut carry = 0u64;
    for i in 0..el {
        out[i] = (v[i] << bits) | carry;
        carry = v[i] >> (64 - bits);
    }
    if carry != 0 {
        out[el] = carry;
        el + 1
    }
    else {
        el
    }
}

pub(crate) fn shr_into(v: &[u64], bits: u32, out: &mut [u64]) -> usize {
    let el = effective_len(v);
    out.fill(0);
    if bits == 0 || is_zero(v) {
        out[..el].copy_from_slice(&v[..el]);
        return el.max(1);
    }
    let mut carry = 0u128;
    for i in (0..el).rev() {
        let wide = u128::from(v[i]) | (carry << 64);
        out[i] = (wide >> bits) as u64;
        carry = wide & ((1u128 << bits) - 1);
    }
    trim_slice_len(out).max(1)
}

/// Knuth Algorithm D for normalized multi-limb division.
///
/// Left-shifting makes the divisor's top bit 1, so the quotient digit estimated
/// from the dividend's top two limbs differs from the true digit by at most a
/// small correction. After subtracting `q̂·v`, a borrow proves `q̂` was one too
/// large; decrement and add `v` back. The final right shift restores the original
/// scale. The invariant is `u = q·v + r` with `0 ≤ r < v`. Division by zero,
/// insufficient output capacity, and budget overflow are rejected before writes.
pub(super) fn div_rem_knuth_into(
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

    let need = div_scratch_limbs(u_el, n);
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

    let u_len = if shift > 0 {
        shl_into(u, shift, u_work)
    }
    else {
        u_work[..u_el].copy_from_slice(&u[..u_el]);
        u_el
    };
    let _ = u_len;
    if shift > 0 {
        let _ = shl_into(v, shift, v_work);
    }
    else {
        v_work.copy_from_slice(&v[..n]);
    }
    // ensure u has m+n+1 limbs
    if u_work.len() > m + n + 1 {
        // truncated by split
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

        let borrow = submul_1_inplace_soft(&mut u_work[j..j + n + 1], v_work, qhat);
        if borrow {
            qhat = qhat.wrapping_sub(1);
            let _ = addmul_1_inplace_soft(&mut u_work[j..j + n + 1], v_work, 1);
        }
        q_work[j] = qhat;
    }

    let qn = effective_len(q_work).max(1);
    q_out.copy_canonical(&q_work[..qn], budget)?;

    if shift > 0 {
        let r_len = shr_into(&u_work[..n], shift, r_work);
        r_out.copy_canonical(&r_work[..r_len], budget)?;
    }
    else {
        let rn = effective_len(&u_work[..n]).max(1);
        r_out.copy_canonical(&u_work[..rn], budget)?;
    }
    Ok(())
}
