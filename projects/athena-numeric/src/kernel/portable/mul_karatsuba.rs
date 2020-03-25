//! # 用途
//! Karatsuba 分治乘法，作用于 limb 切片。
//!
//! # 数学模型
//! 拆分 `A = A₀ + A₁ βᵐ`，`B = B₀ + B₁ βᵐ`。三个乘积
//! `Z₀=A₀B₀`，`Z₂=A₁B₁`，`Z₁=(A₀+A₁)(B₀+B₁)-Z₀-Z₂` 重建
//! `AB = Z₀ + Z₁ βᵐ + Z₂ β²ᵐ`。
//!
//! # 推导
//! 由 `(A₀+A₁)(B₀+B₁) = A₀B₀ + A₀B₁ + A₁B₀ + A₁B₁`，减去
//! `Z₀` 与 `Z₂`，用一次乘法代替两次得到交叉项。
//!
//! # 算法步骤
//! 1. 若 `max(la,lb) < MUL_KARATSUBA_THRESHOLD`，回退到 schoolbook。
//! 2. 在 `m = ⌈n/2⌉` 处拆分。
//! 3. 递归求 `Z₀`、`Z₂` 与和积，写入 scratch 布局。
//! 4. 就地减法形成 `Z₁`；按 limb 移位重组。
//!
//! # 前置条件
//! - `out.len() >= la+lb`；scratch 容量由 `karatsuba_scratch_limbs` 给出。
//! - 调用方清零，或接受本函数清空 `out`。
//!
//! # 后置条件
//! - `out` 为乘积（高位可能有零 limb）。
//!
//! # 复杂度
//! 递推 `T(n)=3T(n/2)+O(n)` → 对平衡输入为 `Θ(n^{log₂ 3})`。
//!
//! # 交叉阈值
//! 规划器在高于 `MUL_KARATSUBA_THRESHOLD`、低于 Toom 时选 Karatsuba。
//! 失衡或过短输入因拆分/重组开销不敌 schoolbook。
//!
//! # 失败模式
//! scratch 不足时 `debug_assert`。递归叶子须清零临时 `out` 切片。
//!
//! # 测试
//! `tests/exact/algorithms.rs`、`tests/runtime/kernel_parity.rs`。

use crate::algorithm::MUL_KARATSUBA_THRESHOLD;

use super::{
    mul_schoolbook::mul_schoolbook_into,
    primitive::{effective_len, is_zero},
    slice_ops::{add_assign_shifted, add_slices_into, split_lo_hi, sub_assign_slices, trim_slice_len},
};

/// 递归乘法：`out` 为目标，`scratch` 为剩余工作区（顺序复用）。
///
/// 拆分恒等式为 `(a₀+a₁)(b₀+b₁)−a₀b₀−a₁b₁ = a₀b₁+a₁b₀`。
/// 故三个半尺寸乘积代替四个。`out` 须清零且容纳
/// `a.len()+b.len()` 个 limb。scratch 由调用方持有，因递归临时
/// 分配会抹掉渐近收益。交叉点刻意高于 schoolbook 区间：
/// 递归增加线性时间的拆分、求和、减法与重组，故对过短或
/// 严重失衡操作数反而更慢，尽管递推为 `Θ(n^{log₂ 3})`。
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
