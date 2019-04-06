//! # Purpose
//! Karatsuba divide-and-conquer multiplication on limb slices.
//!
//! # Mathematical model
//! Split $A = A_0 + A_1 \beta^m$, $B = B_0 + B_1 \beta^m$. Three products
//! $Z_0=A_0 B_0$, $Z_2=A_1 B_1$, $Z_1=(A_0+A_1)(B_0+B_1)-Z_0-Z_2$ rebuild
//! $A B = Z_0 + Z_1 \beta^m + Z_2 \beta^{2m}$.
//!
//! # Derivation
//! From $(A_0+A_1)(B_0+B_1) = A_0 B_0 + A_0 B_1 + A_1 B_0 + A_1 B_1$, subtract
//! $Z_0$ and $Z_2$ to isolate the cross term with one multiply instead of two.
//!
//! # Algorithm steps
//! 1. If max(la,lb) < MUL_KARATSUBA_THRESHOLD, fall back to schoolbook.
//! 2. Split at $m = \lceil n/2 \rceil$.
//! 3. Recurse for $Z_0$, $Z_2$, and the sum-product into scratch layout.
//! 4. Form $Z_1$ by in-place subtract; recompose with limb shifts.
//!
//! # Preconditions
//! - out.len() >= la+lb; scratch sized by karatsuba_scratch_limbs.
//! - Caller zeros or accepts that this function clears out.
//!
//! # Postconditions
//! - out is the product (possibly with high zero limbs).
//!
//! # Complexity
//! Recurrence $T(n)=3T(n/2)+O(n)$ → $\Theta(n^{\log_2 3})$ for balanced inputs.
//!
//! # Crossover
//! Planner selects Karatsuba above MUL_KARATSUBA_THRESHOLD and below Toom.
//! Unbalanced or short inputs lose to schoolbook due to split/recombine overhead.
//!
//! # Failure modes
//! Scratch underrun debug_assert. Recursive leaves must clear temporary out slices.
//!
//! # Tests
//! `tests/exact/algorithms.rs`, `tests/runtime/kernel_parity.rs`.

use crate::algorithm::MUL_KARATSUBA_THRESHOLD;

use super::{
    mul_schoolbook::mul_schoolbook_into,
    primitive::{effective_len, is_zero},
    slice_ops::{add_assign_shifted, add_slices_into, split_lo_hi, sub_assign_slices, trim_slice_len},
};

/// 递归乘法：`out` 为目标，`scratch` 为剩余工作区（顺序复用）。
/// Recursive Karatsuba multiplication.
///
/// The split identity is `(a₀+a₁)(b₀+b₁)−a₀b₀−a₁b₁ = a₀b₁+a₁b₀`.
/// Thus three half-size products replace four. `out` must be zeroed and hold
/// `a.len()+b.len()` limbs. Scratch is caller-owned because recursive temporary
/// allocation would erase the asymptotic win. The crossover is deliberately
/// above the schoolbook range: recursion adds linear-time splitting, sums,
/// subtraction and recomposition, so it loses for short or very unbalanced
/// operands even though its recurrence is Θ(nˡᵒᵍ²³).
pub(super) fn mul_rec(a: &[u64], b: &[u64], out: &mut [u64], scratch: &mut [u64]) {
    let la = effective_len(a);
    let lb = effective_len(b);
    let need = (la + lb).max(1);
    debug_assert!(out.len() >= need);
    // 必须清零整段 `out`：Karatsuba 临时区来自 scratch，高位残留会破坏后续比较/减法。
    out.fill(0);
    if is_zero(a) || is_zero(b) {
        return;
    }
    if la.max(lb) < MUL_KARATSUBA_THRESHOLD {
        mul_schoolbook_into(a, b, &mut out[..need]);
        return;
    }

    let n = la.max(lb);
    let m = (n + 1) / 2;
    let (al, ah) = split_lo_hi(a, m);
    let (bl, bh) = split_lo_hi(b, m);

    let z0_len = 2 * m;
    let z2_len = 2 * m;
    let asum_len = m + 1;
    let bsum_len = m + 1;
    let z1_len = 2 * m + 2;
    let level = z0_len + z2_len + asum_len + bsum_len + z1_len;
    debug_assert!(scratch.len() >= level, "karatsuba scratch underrun");

    let (level_scratch, rest) = scratch.split_at_mut(level);
    let (z0, rest_l) = level_scratch.split_at_mut(z0_len);
    let (z2, rest_l) = rest_l.split_at_mut(z2_len);
    let (asum, rest_l) = rest_l.split_at_mut(asum_len);
    let (bsum, rest_l) = rest_l.split_at_mut(bsum_len);
    let z1 = rest_l;

    mul_rec(al, bl, z0, rest);
    mul_rec(ah, bh, z2, rest);

    add_slices_into(al, ah, asum);
    add_slices_into(bl, bh, bsum);
    let asum_n = trim_slice_len(asum);
    let bsum_n = trim_slice_len(bsum);
    mul_rec(&asum[..asum_n.max(1)], &bsum[..bsum_n.max(1)], z1, rest);

    // z1 = (al+ah)*(bl+bh) - z0 - z2（就地，无需额外临时）
    {
        sub_assign_slices(z1, z0);
        sub_assign_slices(z1, z2);
    }

    // out = z0 + (z1 << m) + (z2 << 2m)
    out.fill(0);
    add_assign_shifted(out, z0, 0);
    add_assign_shifted(out, z1, m);
    add_assign_shifted(out, z2, 2 * m);
}
