//! # 用途
//! Half-GCD（Jebelean / Lehmer 矩阵风格），用于宽的非负 limb 量级。
//!
//! # 数学模型
//! Euclid 变换保持 `gcd(a,b)`。由前导 limb 商累积的幺模
//! `2×2` 矩阵以带符号线性组合施加。Half-GCD 在较小操作数
//! 约为原宽度一半时停止，再以普通 Lehmer/二进制 GCD 收尾。
//!
//! # 推导
//! 前导双 limb 的 Euclid 在商序列可认证时镜像完整 Euclid；
//! 矩阵乘积随后对完整操作数精确成立。
//! 需要负矩阵元（与保守的非负 Lehmer 路径不同）。
//! 认证失败时，一次精确取余即可恢复进度。
//!
//! # 算法步骤
//! 1. 规范化；交换使 `a ≥ b`。
//! 2. 当 `min(|a|,|b|)` 至少达到 half-GCD 阈值时，经带符号
//!    Lehmer 块（或失败时一次精确 `rem`）向半宽缩减。
//! 3. 以 [`super::gcd_lehmer::gcd`] 收尾。
//!
//! # 前置条件
//! - 规范的非负小端 `u64` limbs（`Vec` 便利路径）。
//!
//! # 后置条件
//! - 返回 `gcd(a,b)` 的规范 limb 向量。
//!
//! # 复杂度
//! 相对纯 Euclid，宽输入上全精度除法更少；渐近
//! 仍由收尾 GCD 主导。
//!
//! # 交叉阈值
//! 当两端操作数至少有 `GCD_LEHMER_THRESHOLD * 4` 个 limb
//! 且设有 `half_gcd` 能力时，规划器选择 HalfGcd。
//!
//! # 失败模式
//! Lehmer 块可能返回 false（前导商不稳定）；外层循环
//! 随后执行一次精确 `div_rem`。
//!
//! # 测试
//! `tests/exact/algorithms.rs` 中 half-GCD 能力交叉校验。

use std::cmp::Ordering;

use super::{
    convenience::{add_n, div_rem, mul_1, sub_n},
    gcd_lehmer::gcd as lehmer_gcd,
    primitive::{cmp_slice, effective_len, is_zero, normalize_trim},
};

/// 与规划器阈值一致（`GCD_LEHMER_THRESHOLD * 4`）。
const HALF_GCD_THRESHOLD: usize = 12;
const LEHMER_THRESHOLD: usize = 3;

/// Half-GCD，再以 Lehmer/二进制收尾。
pub(crate) fn half_gcd(mut a: Vec<u64>, mut b: Vec<u64>) -> Vec<u64> {
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

    while effective_len(&b) >= HALF_GCD_THRESHOLD && effective_len(&a) >= HALF_GCD_THRESHOLD {
        let n0 = effective_len(&a).max(effective_len(&b));
        let target = (n0 + 1) / 2;
        let mut progressed = false;
        while effective_len(&b) > target.max(LEHMER_THRESHOLD) {
            if hgcd_lehmer_block(&mut a, &mut b) {
                a = normalize_trim(a);
                b = normalize_trim(b);
                progressed = true;
            }
            else {
                // 前导预测失败时做一次精确 Euclid 步。
                let (_q, r) = div_rem(&a, &b);
                a = b;
                b = normalize_trim(r);
                progressed = true;
                break;
            }
            if is_zero(&b) {
                return a;
            }
            if cmp_slice(&a, &b) == Ordering::Less {
                std::mem::swap(&mut a, &mut b);
            }
        }
        if !progressed {
            break;
        }
        if is_zero(&b) {
            return a;
        }
        if cmp_slice(&a, &b) == Ordering::Less {
            std::mem::swap(&mut a, &mut b);
        }
    }

    lehmer_gcd(a, b)
}

/// 一次带符号矩阵的 Lehmer 块（Jebelean 风格 HGCD 步片段）。
fn hgcd_lehmer_block(a: &mut Vec<u64>, b: &mut Vec<u64>) -> bool {
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
    let mut steps = 0u32;

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
        steps += 1;
        if steps > 64 {
            break;
        }
    }

    if steps == 0 || y1 == 0 {
        return false;
    }
    if y1.unsigned_abs() > u32::MAX as u64 || x1.unsigned_abs() > u32::MAX as u64 {
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
    if is_zero(&nb_new) {
        *a = na_new;
        *b = nb_new;
        return true;
    }
    // 要求较小操作数不增，以保证进度。
    if effective_len(&nb_new) >= nb && cmp_slice(&nb_new, b) != Ordering::Less {
        return false;
    }
    *a = na_new;
    *b = nb_new;
    if cmp_slice(a, b) == Ordering::Less {
        std::mem::swap(a, b);
    }
    true
}

/// `|c₀|·v₀ ± |c₁|·v₁` 作为非负量级（符号选择加/减）。
fn lincomb_signed(c0: i64, v0: &[u64], c1: i64, v1: &[u64]) -> Option<Vec<u64>> {
    let zero = || vec![0u64];
    let mag = |c: i64, v: &[u64]| -> Vec<u64> { if c == 0 { zero() } else { mul_1(v, c.unsigned_abs()) } };
    let t0 = mag(c0, v0);
    let t1 = mag(c1, v1);
    let s0 = c0 >= 0;
    let s1 = c1 >= 0;
    Some(match (s0, s1) {
        (true, true) => add_n(&t0, &t1),
        (false, false) => add_n(&t0, &t1),
        (true, false) => {
            if cmp_slice(&t0, &t1) == Ordering::Less {
                sub_n(&t1, &t0)
            }
            else {
                sub_n(&t0, &t1)
            }
        }
        (false, true) => {
            if cmp_slice(&t1, &t0) == Ordering::Less {
                sub_n(&t0, &t1)
            }
            else {
                sub_n(&t1, &t0)
            }
        }
    })
}
