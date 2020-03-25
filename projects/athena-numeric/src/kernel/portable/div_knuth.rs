//! # 用途
//! Knuth 算法 D：归一化多 limb 除法，带商修正。
//!
//! # 数学模型
//! 对 `u = q v + r` 且 `0 ≤ r < v`，左移使 `v` 最高位为 1 后，
//! 由工作被除数顶两 limb 估计的每位商数字与真值仅差一个
//! 可修正的小量。
//!
//! # 推导
//! 归一化最大化除数前导 limb。估计
//! `q̂ = ⌊u_{j+n:j+n-1} / v_{n-1}⌋`（封顶）经 `v_{n-2}` 检验
//! 再细化，随后乘减；借位则加回。
//!
//! # 算法步骤
//! 1. `shift` = 除数顶 limb 前导零；将 `u,v` 移入 scratch。
//! 2. 对 `j = m..0`：估计 `q̂`、修正、`submul`、可选加回。
//! 3. 写商；按归一化量右移余数。
//!
//! # 前置条件
//! - `effective_len(v) >= 2`（单 limb 走 `div_single`）。
//! - scratch 容量 `div_scratch_limbs`；预算由调用方/glue 检查。
//!
//! # 后置条件
//! - `u = q v + r`，`0 ≤ r < v`（规范 limbs）。
//!
//! # 复杂度
//! 对 `m+1` 个商 limb 与 `n`-limb 除数，每位步为 `Θ((m+1) n)`。
//!
//! # 交叉阈值
//! 多 limb 除法的规划器默认；仅当被除数远宽时用 BZ。
//!
//! # 失败模式
//! 除零在 glue 中拒绝。预算 / 容量错误以 `Result` 返回。
//!
//! # 测试
//! `tests/exact/algorithms.rs`、自然数 `div_rem` 恒等式套件。

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
        // 仅在 Knuth 归一化且 bits < 64 时使用
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

/// Knuth 算法 D：归一化多 limb 除法。
///
/// 左移使除数最高位为 1，从而由被除数顶两 limb 估计的商数字
/// 与真值至多差一个小修正。减去 `q̂·v` 后若有借位，说明 `q̂`
/// 大了 1；减一并加回 `v`。最终右移恢复原尺度。
/// 不变量为 `u = q·v + r` 且 `0 ≤ r < v`。除零、
/// 输出容量不足与预算溢出在写入前拒绝。
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
    // 确保 u 有 m+n+1 个 limb
    if u_work.len() > m + n + 1 {
        // 已被 split 截断
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
