//! # 用途
//! Burnikel–Ziegler 分块除法：对高半递归，再做一次 Knuth 步。
//!
//! # 数学模型
//! 写 `u = u₁ βⁿ + u₀`，其中 `n = |v|`。用 `v` 除 `u₁` 得
//! `(q₁, r₁)`，构造 `u' = r₁ βⁿ + u₀`，再用 `v` 除 `u'` 得
//! `(q₀, r)`。完整商为 `q₁ βⁿ + q₀`。
//!
//! # 推导
//! 当被除数至少有两个除数宽度块时，遵循被除数以基数 `βⁿ`
//! 表示的 schoolbook 恒等式。
//!
//! # 算法步骤
//! 1. 若 `|u| < 2|v|`，回退到 Knuth。
//! 2. 对高块递归；组装中间值；Knuth 除中间值；合并商。
//!
//! # 前置条件
//! - 多 limb 除数；scratch 与 Knuth 叶子共享。
//!
//! # 后置条件
//! - 与 Knuth 相同：`u = q v + r`，`0 ≤ r < v`。
//!
//! # 复杂度
//! 当 `|u| ≫ |v|` 时，通过减少高部 Knuth 数字循环改善常数。
//!
//! # 交叉阈值
//! `DIV_BZ_THRESHOLD` 与能力位 `bz_division`（`AlgorithmPlanner`）。
//!
//! # 失败模式
//! 经共享辅助与 Knuth 相同的预算 / 除零路径。
//!
//! # 测试
//! `tests/exact/algorithms.rs`（BZ 与 Knuth 能力门控）。

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
