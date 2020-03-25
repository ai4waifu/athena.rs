//! # 用途
//! Lehmer 加速的 Euclid GCD，失败时回退到二进制 GCD。
//!
//! # 数学模型
//! Euclid 算法保持 `gcd(a,b)`。Lehmer 仅用前导 limb 模拟多步商，
//! 累积幺模 `2×2` 矩阵，再以带符号线性组合施加到完整操作数。
//!
//! # 推导
//! 前导 limb 界证明一块商与真 Euclid 商一致；施加矩阵即为
//! 精确的多 limb 更新。认证失败则落入 `binary_gcd`。
//!
//! # 算法步骤
//! 1. 规范化；交换使 `a ≥ b`。
//! 2. 当两端宽度均 `≥ LEHMER_THRESHOLD` 时尝试 `lehmer_step`。
//! 3. 以 `binary_gcd` 收尾。
//!
//! # 前置条件
//! - 规范的非负 limb 量级（当前为 `Vec` 便利路径）。
//!
//! # 后置条件
//! - 返回 `gcd(a,b)` 的规范 limb 向量。
//!
//! # 复杂度
//! 类似 Euclid；Lehmer 成功时全精度除法更少。
//!
//! # 交叉阈值
//! 小操作数跳过 Lehmer，直走二进制 GCD。
//!
//! # 失败模式
//! `lehmer_step` 在前导商不稳定时返回 `false`（非错误）。
//!
//! # 测试
//! `tests/exact/limb_kernel.rs`、`tests/exact/algorithms.rs`。

use std::cmp::Ordering;

use super::{
    convenience::{add_n, mul_1, sub_n},
    gcd_binary::binary_gcd,
    primitive::{cmp_slice, effective_len, is_zero, normalize_trim},
};

const LEHMER_THRESHOLD: usize = 3;

pub(crate) fn gcd(mut a: Vec<u64>, mut b: Vec<u64>) -> Vec<u64> {
    a = normalize_trim(a);
    b = normalize_trim(b);
    if is_zero(&a) {
        return b;
    }
    if is_zero(&b) {
        return a;
    }
    if cmp_slice(&a, &b) == Ordering::Less {
        std::mem::swap(&mut a, &mut b);
    }

    while effective_len(&b) >= LEHMER_THRESHOLD && effective_len(&a) >= LEHMER_THRESHOLD {
        if !lehmer_step(&mut a, &mut b) {
            break;
        }
        a = normalize_trim(a);
        b = normalize_trim(b);
        if is_zero(&b) {
            return a;
        }
        if cmp_slice(&a, &b) == Ordering::Less {
            std::mem::swap(&mut a, &mut b);
        }
    }
    binary_gcd(a, b)
}

/// 仅用前导 limb 施加一块 Euclid 商的 Lehmer 步。
///
/// `2×2` 矩阵在前导 limb 界证明稳定时累积候选商，再以带符号
/// 线性组合施加到完整操作数。返回 `false` 表示预测未认证
/// 或无进度，调用方须做一次精确取余。因幺模 Euclid
/// 变换保持公因子集合，故仍保持 gcd。
fn lehmer_step(a: &mut Vec<u64>, b: &mut Vec<u64>) -> bool {
    let na = effective_len(a);
    let nb = effective_len(b);
    if nb < 2 || na < nb {
        return false;
    }
    let n = na - nb;

    let u1 = *a.get(nb + n - 1).unwrap_or(&0);
    let u0 = *a.get(nb + n - 2).unwrap_or(&0);
    let v1 = b[nb - 1];
    let v0 = if nb >= 2 { b[nb - 2] } else { 0 };
    if v1 == 0 {
        return false;
    }

    let mut x0: i64 = 1;
    let mut x1: i64 = 0;
    let mut y0: i64 = 0;
    let mut y1: i64 = 1;
    let mut uh = (u128::from(u1) << 64) | u128::from(u0);
    let mut vh = (u128::from(v1) << 64) | u128::from(v0);

    while vh >= (1u128 << 63) {
        let q = uh / vh;
        let r = uh % vh;
        if r < (1u128 << 63) {
            break;
        }
        let t = x0 as i128 - (q as i128) * (x1 as i128);
        if t < i64::MIN as i128 || t > i64::MAX as i128 {
            break;
        }
        x0 = x1;
        x1 = t as i64;
        let t = y0 as i128 - (q as i128) * (y1 as i128);
        if t < i64::MIN as i128 || t > i64::MAX as i128 {
            break;
        }
        y0 = y1;
        y1 = t as i64;
        uh = vh;
        vh = r;
    }

    if y1 == 0 || y1.unsigned_abs() > u32::MAX as u64 {
        return false;
    }
    if x0 < 0 || x1 < 0 || y0 < 0 || y1 < 0 {
        return false;
    }

    let Some(na_new) = lincomb_signed(x0, a, x1, b)
    else {
        return false;
    };
    let Some(nb_new) = lincomb_signed(y0, a, y1, b)
    else {
        return false;
    };
    if is_zero(&na_new) || is_zero(&nb_new) || cmp_slice(&na_new, &nb_new) == Ordering::Less {
        return false;
    }
    *a = na_new;
    *b = nb_new;
    true
}

fn lincomb_signed(c0: i64, v0: &[u64], c1: i64, v1: &[u64]) -> Option<Vec<u64>> {
    let zero = || vec![0u64];
    let t0 = if c0 == 0 {
        zero()
    }
    else if c0 > 0 {
        mul_1(v0, c0 as u64)
    }
    else {
        return None;
    };
    let t1 = if c1 == 0 {
        zero()
    }
    else if c1 > 0 {
        mul_1(v1, c1 as u64)
    }
    else {
        return None;
    };
    Some(match (c0 >= 0, c1 >= 0) {
        (true, true) => add_n(&t0, &t1),
        (true, false) => {
            if cmp_slice(&t0, &t1) == Ordering::Less {
                return None;
            }
            sub_n(&t0, &t1)
        }
        (false, true) => {
            if cmp_slice(&t1, &t0) == Ordering::Less {
                return None;
            }
            sub_n(&t1, &t0)
        }
        (false, false) => add_n(&t0, &t1),
    })
}
