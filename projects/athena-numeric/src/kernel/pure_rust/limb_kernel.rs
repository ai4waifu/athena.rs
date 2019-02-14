//! 纯 Rust machine kernel：固定宽度原语 + 通用 `*_into`。
//!
//! 只写调用方自有 [`LimbBuffer`] / scratch。算法策略与分配/GC 不在本层。
//! 禁止「算完再吞并新 `Vec`」作为值层主路径（`*_budgeted` 仅便利/测试）。

use athena_types::Result;

use crate::policy::execution_budget::ExecutionBudget;

use crate::kernel::{LimbBuffer, ScratchWorkspace, kernel_err};

use std::{cell::RefCell, cmp::Ordering};

thread_local! {
    static KERNEL_SCRATCH: RefCell<ScratchWorkspace> = RefCell::new(ScratchWorkspace::default());
}

/// 借用线程本地 scratch 执行 kernel（调用结束清空）。
pub(crate) fn with_kernel_scratch<R>(
    budget: &ExecutionBudget,
    f: impl FnOnce(&mut ScratchWorkspace, &ExecutionBudget) -> R,
) -> R {
    KERNEL_SCRATCH.with(|cell| {
        if let Ok(mut scratch) = cell.try_borrow_mut() {
            let result = f(&mut *scratch, budget);
            scratch.clear();
            result
        }
        else {
            let mut scratch = ScratchWorkspace::default();
            f(&mut scratch, budget)
        }
    })
}

/// Karatsuba / Toom 乘法阈值（每操作数 limb 数）。
pub(crate) use crate::algorithm::{MUL_KARATSUBA_THRESHOLD, MUL_TOOM_THRESHOLD, karatsuba_scratch_limbs, toom3_scratch_limbs};

/// 全宽单 limb 乘积：`(hi, lo) = a * b`。
#[inline]
pub(crate) fn mul_wide(a: u64, b: u64) -> (u64, u64) {
    let prod = (a as u128) * (b as u128);
    ((prod >> 64) as u64, prod as u64)
}

/// 带进位加法：`(sum, carry_out) = a + b + carry_in`。
#[inline]
pub(crate) fn adc(a: u64, b: u64, carry: u64) -> (u64, u64) {
    let sum = (a as u128) + (b as u128) + (carry as u128);
    (sum as u64, (sum >> 64) as u64)
}

/// 带借位减法：`(diff, borrow_out) = a - b - borrow_in`。
#[inline]
pub(crate) fn sbb(a: u64, b: u64, borrow: u64) -> (u64, u64) {
    let sub = (b as u128) + (borrow as u128);
    let a128 = a as u128;
    if a128 >= sub { ((a128 - sub) as u64, 0) } else { ((a128 + (1u128 << 64) - sub) as u64, 1) }
}

/// 融合乘加写入 limb：`(limb, carry) = acc + a * b + carry`。
#[inline]
pub(crate) fn mac(acc: u64, a: u64, b: u64, carry: u128) -> (u64, u128) {
    let sum = (acc as u128) + (a as u128) * (b as u128) + carry;
    (sum as u64, sum >> 64)
}

// --- 固定宽度合同（1/2 limb；禁止走通用循环 / Karatsuba）---

/// `Limb1 + Limb1`：`(lo, carry)`，`carry ∈ {0,1}`。
#[inline]
pub(crate) fn add_1(a: u64, b: u64) -> (u64, u64) {
    let (sum, carry) = a.overflowing_add(b);
    (sum, u64::from(carry))
}

/// `Limb1 × Limb1`：全宽积（`u128` 仅作累加器，非存储）。
#[inline]
pub(crate) fn mul_1x1(a: u64, b: u64) -> u128 {
    (a as u128) * (b as u128)
}

/// `Limb1 + Limb2`：最多 3 limbs；返回 `(limbs, effective_len)`。
#[inline]
pub(crate) fn add_1_2(a: u64, b: [u64; 2]) -> ([u64; 3], usize) {
    let (lo, c0) = adc(a, b[0], 0);
    let (hi, c1) = adc(b[1], 0, c0);
    if c1 == 0 { if hi == 0 { ([lo, 0, 0], if lo == 0 { 0 } else { 1 }) } else { ([lo, hi, 0], 2) } } else { ([lo, hi, c1], 3) }
}

/// `Limb2 + Limb2`：双 limb adc；进位则 3 limbs。
#[inline]
pub(crate) fn add_2(a: [u64; 2], b: [u64; 2]) -> ([u64; 3], usize) {
    let (lo, c0) = adc(a[0], b[0], 0);
    let (hi, c1) = adc(a[1], b[1], c0);
    if c1 == 0 { if hi == 0 { ([lo, 0, 0], if lo == 0 { 0 } else { 1 }) } else { ([lo, hi, 0], 2) } } else { ([lo, hi, c1], 3) }
}

/// `Limb2 × Limb1`：最多 3 limbs。
#[inline]
pub(crate) fn mul_2x1(a: [u64; 2], b: u64) -> ([u64; 3], usize) {
    if b == 0 {
        return ([0, 0, 0], 0);
    }
    // `mul_wide` → (hi, lo)
    let (hi0, lo) = mul_wide(a[0], b);
    let (hi1, mid_lo) = mul_wide(a[1], b);
    let (mid, c1) = adc(mid_lo, hi0, 0);
    let hi = hi1 + c1;
    if hi == 0 {
        if mid == 0 { ([lo, 0, 0], if lo == 0 { 0 } else { 1 }) } else { ([lo, mid, 0], 2) }
    }
    else {
        ([lo, mid, hi], 3)
    }
}

/// `Limb1 − Limb1`（要求 `a >= b`）。
#[inline]
pub(crate) fn sub_1(a: u64, b: u64) -> u64 {
    debug_assert!(a >= b);
    a.wrapping_sub(b)
}

/// `Limb2 − Limb1`（要求 `a >= b`）；返回 `(limbs, effective_len)`。
#[inline]
pub(crate) fn sub_2_1(a: [u64; 2], b: u64) -> ([u64; 2], usize) {
    let (lo, br0) = sbb(a[0], b, 0);
    let (hi, br1) = sbb(a[1], 0, br0);
    debug_assert!(br1 == 0, "sub_2_1 underflow");
    if hi == 0 { ([lo, 0], if lo == 0 { 0 } else { 1 }) } else { ([lo, hi], 2) }
}

/// `Limb2 − Limb2`（要求 `a >= b`）。
#[inline]
pub(crate) fn sub_2(a: [u64; 2], b: [u64; 2]) -> ([u64; 2], usize) {
    let (lo, br0) = sbb(a[0], b[0], 0);
    let (hi, br1) = sbb(a[1], b[1], br0);
    debug_assert!(br1 == 0, "sub_2 underflow");
    if hi == 0 { ([lo, 0], if lo == 0 { 0 } else { 1 }) } else { ([lo, hi], 2) }
}

/// `Limb1 ÷ Limb1`：`(q, r)`，`b != 0`。
#[inline]
pub(crate) fn div_rem_1(a: u64, b: u64) -> (u64, u64) {
    debug_assert!(b != 0);
    (a / b, a % b)
}

/// 至多两 limb 的无符号值除法（两侧均可落入 `u128`）。
#[inline]
pub(crate) fn div_rem_u128(a: u128, b: u128) -> (u128, u128) {
    debug_assert!(b != 0);
    (a / b, a % b)
}

/// `[lo, hi]` → `u128`（`hi` 可为 0）。
#[inline]
pub(crate) fn limbs2_to_u128(limbs: [u64; 2]) -> u128 {
    (limbs[0] as u128) | ((limbs[1] as u128) << 64)
}

/// `Limb2 ÷ Limb1`（`d != 0`）：商最多 2 limbs。
#[inline]
pub(crate) fn div_rem_2_1(u: [u64; 2], d: u64) -> ([u64; 2], usize, u64) {
    debug_assert!(d != 0);
    let n = limbs2_to_u128(u);
    let q = n / (d as u128);
    let r = (n % (d as u128)) as u64;
    let lo = q as u64;
    let hi = (q >> 64) as u64;
    if hi == 0 { ([lo, 0], if lo == 0 { 0 } else { 1 }, r) } else { ([lo, hi], 2, r) }
}

/// `Limb2 × Limb2`：最多 4 limbs。
#[inline]
pub(crate) fn mul_2(a: [u64; 2], b: [u64; 2]) -> ([u64; 4], usize) {
    let p00 = mul_1x1(a[0], b[0]);
    let p01 = mul_1x1(a[0], b[1]);
    let p10 = mul_1x1(a[1], b[0]);
    let p11 = mul_1x1(a[1], b[1]);

    let mut out = [0u64; 4];
    out[0] = p00 as u64;
    let mut carry = p00 >> 64;

    let t1 = (p01 as u64 as u128) + (p10 as u64 as u128) + carry;
    out[1] = t1 as u64;
    carry = t1 >> 64;

    let t2 = (p01 >> 64) + (p10 >> 64) + (p11 as u64 as u128) + carry;
    out[2] = t2 as u64;
    carry = t2 >> 64;

    let t3 = (p11 >> 64) + carry;
    out[3] = t3 as u64;
    debug_assert!(t3 >> 64 == 0);

    let mut len = 4;
    while len > 0 && out[len - 1] == 0 {
        len -= 1;
    }
    (out, len)
}

pub(crate) fn is_zero(v: &[u64]) -> bool {
    effective_len(v) == 1 && v[0] == 0
}

pub(crate) fn effective_len(v: &[u64]) -> usize {
    let mut n = v.len();
    while n > 1 && v[n - 1] == 0 {
        n -= 1;
    }
    n
}

pub(crate) fn normalize_trim(mut v: Vec<u64>) -> Vec<u64> {
    while v.len() > 1 && v.last() == Some(&0) {
        v.pop();
    }
    if v.is_empty() {
        v.push(0);
    }
    v
}

pub(crate) fn cmp_slice(a: &[u64], b: &[u64]) -> Ordering {
    let la = effective_len(a);
    let lb = effective_len(b);
    match la.cmp(&lb) {
        Ordering::Equal => {}
        other => return other,
    }
    for i in (0..la).rev() {
        match a[i].cmp(&b[i]) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

/// Knuth 除法 scratch：归一化 u、v、商，以及可选余数右移缓冲。
fn div_scratch_limbs(u_limbs: usize, v_limbs: usize) -> usize {
    let m = u_limbs.saturating_sub(v_limbs);
    (m + v_limbs + 1) + v_limbs + (m + 1) + v_limbs
}

fn trim_slice_len(v: &mut [u64]) -> usize {
    let mut n = v.len();
    while n > 1 && v[n - 1] == 0 {
        n -= 1;
    }
    n
}

fn split_lo_hi(v: &[u64], mid: usize) -> (&[u64], &[u64]) {
    static ZERO: [u64; 1] = [0];
    let el = effective_len(v);
    if el == 0 {
        return (&ZERO, &ZERO);
    }
    if mid >= el {
        return (&v[..el], &ZERO);
    }
    let lo = &v[..mid];
    let hi = &v[mid..el];
    let lo = if effective_len(lo) == 0 { &ZERO[..] } else { lo };
    let hi = if hi.is_empty() || effective_len(hi) == 0 { &ZERO[..] } else { hi };
    (lo, hi)
}

fn add_slices_into(a: &[u64], b: &[u64], out: &mut [u64]) {
    let n = effective_len(a).max(effective_len(b));
    debug_assert!(out.len() >= n + 1);
    out.fill(0);
    let mut carry = 0u64;
    for i in 0..n {
        let (sum, c) = adc(*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0), carry);
        out[i] = sum;
        carry = c;
    }
    out[n] = carry;
}

fn sub_slices_into(a: &[u64], b: &[u64], out: &mut [u64]) {
    debug_assert!(cmp_slice(a, b) != Ordering::Less);
    let n = effective_len(a);
    debug_assert!(out.len() >= n);
    out.fill(0);
    let mut borrow = 0u64;
    for i in 0..n {
        let (diff, b_out) = sbb(*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0), borrow);
        out[i] = diff;
        borrow = b_out;
    }
}

fn sub_assign_slices(a: &mut [u64], b: &[u64]) {
    debug_assert!(cmp_slice(a, b) != Ordering::Less);
    let n = effective_len(a).max(effective_len(b));
    let mut borrow = 0u64;
    for i in 0..n {
        let ai = if i < a.len() { a[i] } else { 0 };
        let (diff, b_out) = sbb(ai, *b.get(i).unwrap_or(&0), borrow);
        if i < a.len() {
            a[i] = diff;
        }
        borrow = b_out;
    }
    debug_assert_eq!(borrow, 0);
}

fn add_assign_shifted(out: &mut [u64], src: &[u64], shift_limbs: usize) {
    let sn = effective_len(src);
    if sn == 0 || is_zero(src) {
        return;
    }
    let mut carry = 0u64;
    for i in 0..sn {
        let idx = i + shift_limbs;
        if idx >= out.len() {
            break;
        }
        let (sum, c) = adc(out[idx], src[i], carry);
        out[idx] = sum;
        carry = c;
    }
    let mut idx = sn + shift_limbs;
    while carry > 0 && idx < out.len() {
        let (sum, c) = adc(out[idx], 0, carry);
        out[idx] = sum;
        carry = c;
        idx += 1;
    }
}

/// 小学乘法写入 `out`（长度至少 `la + lb`）。
pub(crate) fn mul_schoolbook_into(a: &[u64], b: &[u64], out: &mut [u64]) {
    let la = effective_len(a);
    let lb = effective_len(b);
    let need = la + lb;
    debug_assert!(out.len() >= need.max(1));
    out[..need.max(1)].fill(0);
    if la == 0 || lb == 0 || is_zero(a) || is_zero(b) {
        return;
    }
    for (i, &av) in a.iter().take(la).enumerate() {
        let mut carry = 0u128;
        for (j, &bv) in b.iter().take(lb).enumerate() {
            let idx = i + j;
            let (limb, c) = mac(out[idx], av, bv, carry);
            out[idx] = limb;
            carry = c;
        }
        let mut k = i + lb;
        while carry > 0 && k < out.len() {
            let sum = u128::from(out[k]) + carry;
            out[k] = sum as u64;
            carry = sum >> 64;
            k += 1;
        }
    }
}

fn mul_1_into_slice(a: &[u64], limb: u64, out: &mut [u64]) {
    let la = effective_len(a);
    debug_assert!(out.len() >= la + 1);
    out.fill(0);
    if limb == 0 || is_zero(a) {
        return;
    }
    if limb == 1 {
        out[..la].copy_from_slice(&a[..la]);
        return;
    }
    let mut carry = 0u128;
    for i in 0..la {
        let prod = u128::from(a[i]) * u128::from(limb) + carry;
        out[i] = prod as u64;
        carry = prod >> 64;
    }
    if carry > 0 {
        out[la] = carry as u64;
    }
}

fn sqr_schoolbook_into(a: &[u64], out: &mut [u64]) {
    let la = effective_len(a);
    if la == 0 || is_zero(a) {
        out.fill(0);
        return;
    }
    if la == 1 {
        mul_1_into_slice(a, a[0], out);
        return;
    }
    let need = 2 * la;
    debug_assert!(out.len() >= need);
    out[..need].fill(0);
    for i in 0..la {
        for j in i..la {
            let (hi, lo) = mul_wide(a[i], a[j]);
            let prod = (u128::from(hi) << 64) | u128::from(lo);
            let idx = i + j;
            add_wide_to_slice(out, idx, prod);
            if i != j {
                add_wide_to_slice(out, idx, prod);
            }
        }
    }
}

fn add_wide_to_slice(out: &mut [u64], idx: usize, wide: u128) {
    let mut carry = wide;
    let mut k = idx;
    while carry > 0 && k < out.len() {
        let sum = u128::from(out[k]) + carry;
        out[k] = sum as u64;
        carry = sum >> 64;
        k += 1;
    }
}

/// 递归乘法：`out` 为目标，`scratch` 为剩余工作区（顺序复用）。
fn mul_rec(a: &[u64], b: &[u64], out: &mut [u64], scratch: &mut [u64]) {
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

/// Toom-3（Bodrato）：五点求值 `0,1,-1,2,∞` + 插值；子乘积走 `mul_rec`（无 `Vec`）。
fn toom3_mul_rec(a: &[u64], b: &[u64], out: &mut [u64], scratch: &mut [u64]) {
    let la = effective_len(a);
    let lb = effective_len(b);
    let need = (la + lb).max(1);
    debug_assert!(out.len() >= need);
    out.fill(0);
    if is_zero(a) || is_zero(b) {
        return;
    }
    let n = la.max(lb);
    if n < MUL_TOOM_THRESHOLD {
        mul_rec(a, b, out, scratch);
        return;
    }
    let m = (n + 2) / 3;
    let (a0, a1, a2) = split_three(a, m);
    let (b0, b1, b2) = split_three(b, m);

    let eval_len = m + 2;
    let prod_len = 2 * eval_len;
    let level = 5 * prod_len + 11 * eval_len + 4 * prod_len;
    debug_assert!(scratch.len() >= level, "toom3 scratch underrun");
    let (level_scratch, rest) = scratch.split_at_mut(level);
    let (prods, rem) = level_scratch.split_at_mut(5 * prod_len);
    let (evals, interp) = rem.split_at_mut(11 * eval_len);

    let (p0, r) = prods.split_at_mut(prod_len);
    let (p1, r) = r.split_at_mut(prod_len);
    let (pm1, r) = r.split_at_mut(prod_len);
    let (p2, pinf) = r.split_at_mut(prod_len);

    let (ea, rem) = evals.split_at_mut(5 * eval_len);
    let (eb, etmp) = rem.split_at_mut(5 * eval_len);
    let (ea0, r) = ea.split_at_mut(eval_len);
    let (ea1, r) = r.split_at_mut(eval_len);
    let (ea_m1, r) = r.split_at_mut(eval_len);
    let (ea2, ea_inf) = r.split_at_mut(eval_len);
    let (eb0, r) = eb.split_at_mut(eval_len);
    let (eb1, r) = r.split_at_mut(eval_len);
    let (eb_m1, r) = r.split_at_mut(eval_len);
    let (eb2, eb_inf) = r.split_at_mut(eval_len);

    toom_copy(ea0, a0);
    toom_copy(ea_inf, a2);
    toom_copy(eb0, b0);
    toom_copy(eb_inf, b2);

    ea1.fill(0);
    toom_add_assign(ea1, a0);
    toom_add_assign(ea1, a1);
    toom_add_assign(ea1, a2);
    eb1.fill(0);
    toom_add_assign(eb1, b0);
    toom_add_assign(eb1, b1);
    toom_add_assign(eb1, b2);

    ea2.fill(0);
    toom_add_assign(ea2, a0);
    addmul_1_inplace(ea2, a1, 2);
    addmul_1_inplace(ea2, a2, 4);
    eb2.fill(0);
    toom_add_assign(eb2, b0);
    addmul_1_inplace(eb2, b1, 2);
    addmul_1_inplace(eb2, b2, 4);

    let sa = toom_eval_pm1(a0, a1, a2, ea_m1, etmp);
    let sb = toom_eval_pm1(b0, b1, b2, eb_m1, etmp);
    let sp = sa * sb;

    for p in [&mut *p0, &mut *p1, &mut *pm1, &mut *p2, &mut *pinf] {
        p.fill(0);
    }
    mul_rec(toom_trim(ea0), toom_trim(eb0), p0, rest);
    mul_rec(toom_trim(ea1), toom_trim(eb1), p1, rest);
    mul_rec(toom_trim(ea_m1), toom_trim(eb_m1), pm1, rest);
    mul_rec(toom_trim(ea2), toom_trim(eb2), p2, rest);
    mul_rec(toom_trim(ea_inf), toom_trim(eb_inf), pinf, rest);

    toom_interpolate_bodrato(out, m, p0, p1, pm1, sp, p2, pinf, interp);
}

fn toom_eval_pm1(a0: &[u64], a1: &[u64], a2: &[u64], dst: &mut [u64], tmp: &mut [u64]) -> i8 {
    tmp.fill(0);
    toom_add_assign(tmp, a0);
    toom_add_assign(tmp, a2);
    if cmp_slice(tmp, a1) != Ordering::Less {
        toom_copy(dst, tmp);
        sub_assign_slices(dst, a1);
        1
    }
    else {
        toom_copy(dst, a1);
        sub_assign_slices(dst, tmp);
        -1
    }
}

fn toom_interpolate_bodrato(
    out: &mut [u64],
    m: usize,
    r0: &[u64],
    r1: &[u64],
    rm1: &[u64],
    sm1: i8,
    r2: &[u64],
    rinf: &[u64],
    scratch: &mut [u64],
) {
    let w = r0.len().max(r1.len()).max(rm1.len()).max(r2.len()).max(rinf.len());
    debug_assert!(scratch.len() >= 4 * w);
    let (t1, rest) = scratch.split_at_mut(w);
    let (t2, rest) = rest.split_at_mut(w);
    let (tmp, c3) = rest.split_at_mut(w);

    if sm1 >= 0 {
        toom_copy(t2, r1);
        toom_add_assign(t2, rm1);
        toom_copy(t1, r1);
        debug_assert!(cmp_slice(t1, rm1) != Ordering::Less);
        sub_assign_slices(t1, rm1);
    }
    else {
        toom_copy(t2, r1);
        debug_assert!(cmp_slice(t2, rm1) != Ordering::Less);
        sub_assign_slices(t2, rm1);
        toom_copy(t1, r1);
        toom_add_assign(t1, rm1);
    }
    toom_shr1(t1);
    toom_shr1(t2);
    debug_assert!(cmp_slice(t2, r0) != Ordering::Less);
    sub_assign_slices(t2, r0);
    debug_assert!(cmp_slice(t2, rinf) != Ordering::Less);
    sub_assign_slices(t2, rinf);

    toom_copy(tmp, r2);
    debug_assert!(cmp_slice(tmp, r0) != Ordering::Less);
    sub_assign_slices(tmp, r0);
    toom_copy(c3, rinf);
    toom_shl_bits(c3, 4);
    debug_assert!(cmp_slice(tmp, c3) != Ordering::Less);
    sub_assign_slices(tmp, c3);
    toom_copy(c3, t2);
    toom_shl_bits(c3, 2);
    debug_assert!(cmp_slice(tmp, c3) != Ordering::Less);
    sub_assign_slices(tmp, c3);
    toom_shr1(tmp);

    debug_assert!(cmp_slice(tmp, t1) != Ordering::Less);
    toom_copy(c3, tmp);
    sub_assign_slices(c3, t1);
    toom_divexact(c3, 3);
    debug_assert!(cmp_slice(t1, c3) != Ordering::Less);
    sub_assign_slices(t1, c3);

    out.fill(0);
    add_assign_shifted(out, r0, 0);
    add_assign_shifted(out, t1, m);
    add_assign_shifted(out, t2, 2 * m);
    add_assign_shifted(out, c3, 3 * m);
    add_assign_shifted(out, rinf, 4 * m);
}

fn toom_copy(dst: &mut [u64], src: &[u64]) {
    dst.fill(0);
    let n = effective_len(src).min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
}

fn toom_trim(v: &[u64]) -> &[u64] {
    static ZERO: [u64; 1] = [0];
    let n = effective_len(v);
    if n == 0 { &ZERO } else { &v[..n] }
}

fn toom_add_assign(dst: &mut [u64], src: &[u64]) {
    let n = effective_len(src);
    let mut carry = 0u64;
    for i in 0..n {
        let (sum, c) = adc(dst.get(i).copied().unwrap_or(0), src[i], carry);
        if i < dst.len() {
            dst[i] = sum;
        }
        carry = c;
    }
    let mut i = n;
    while carry > 0 && i < dst.len() {
        let (sum, c) = adc(dst[i], 0, carry);
        dst[i] = sum;
        carry = c;
        i += 1;
    }
}

fn toom_shr1(v: &mut [u64]) {
    let mut carry = 0u64;
    for i in (0..v.len()).rev() {
        let x = v[i];
        v[i] = (x >> 1) | (carry << 63);
        carry = x & 1;
    }
    debug_assert_eq!(carry, 0, "toom3 shr1");
}

fn toom_shl_bits(v: &mut [u64], bits: u32) {
    debug_assert!((1..=63).contains(&bits));
    let mut carry = 0u64;
    for limb in v.iter_mut() {
        let x = *limb;
        *limb = (x << bits) | carry;
        carry = x >> (64 - bits);
    }
    debug_assert_eq!(carry, 0, "toom3 shl overflow");
}

fn toom_divexact(v: &mut [u64], d: u64) {
    debug_assert!(d > 1);
    let mut rem = 0u128;
    for i in (0..v.len()).rev() {
        let cur = (rem << 64) | u128::from(v[i]);
        v[i] = (cur / u128::from(d)) as u64;
        rem = cur % u128::from(d);
    }
    debug_assert_eq!(rem, 0, "toom3 divexact");
}

fn split_three(v: &[u64], m: usize) -> (&[u64], &[u64], &[u64]) {
    static ZERO: [u64; 1] = [0];
    let el = effective_len(v);
    if el == 0 {
        return (&ZERO, &ZERO, &ZERO);
    }
    let p0 = if m >= el { &v[..el] } else { &v[..m] };
    let p1 = if m >= el {
        &ZERO[..]
    }
    else if 2 * m >= el {
        &v[m..el]
    }
    else {
        &v[m..2 * m]
    };
    let p2 = if 2 * m >= el { &ZERO[..] } else { &v[2 * m..el] };
    let p0 = if effective_len(p0) == 0 { &ZERO[..] } else { p0 };
    let p1 = if p1.is_empty() || effective_len(p1) == 0 { &ZERO[..] } else { p1 };
    let p2 = if p2.is_empty() || effective_len(p2) == 0 { &ZERO[..] } else { p2 };
    (p0, p1, p2)
}

/// Burnikel–Ziegler：大被除数按除数宽度切块递归；小情况回退 Knuth。
fn div_rem_bz_into(
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

/// 就地 `r += a * n`（单 limb `n`）。
pub(crate) fn addmul_1_inplace(r: &mut [u64], a: &[u64], n: u64) -> u64 {
    if n == 0 || is_zero(a) {
        return 0;
    }
    let la = effective_len(a);
    let mut carry = 0u128;
    for i in 0..la {
        let prod = u128::from(a[i]) * u128::from(n) + u128::from(r.get(i).copied().unwrap_or(0)) + carry;
        if i < r.len() {
            r[i] = prod as u64;
        }
        carry = prod >> 64;
    }
    let mut idx = la;
    while carry > 0 {
        if idx >= r.len() {
            break;
        }
        let sum = u128::from(r[idx]) + carry;
        r[idx] = sum as u64;
        carry = sum >> 64;
        idx += 1;
    }
    carry as u64
}

/// 从 `r` 减去 `a * n`；发生借位（下溢）时返回 `true`。融合路径，不分配。
pub(crate) fn submul_1_inplace(r: &mut [u64], a: &[u64], n: u64) -> bool {
    if n == 0 || is_zero(a) {
        return false;
    }
    let la = effective_len(a);
    let mut borrow = 0u64;
    let mut carry_hi = 0u64;
    for i in 0..r.len() {
        let av = if i < la { a[i] } else { 0 };
        let prod = u128::from(av) * u128::from(n) + u128::from(carry_hi);
        let plo = prod as u64;
        carry_hi = (prod >> 64) as u64;
        let (diff, b_out) = sbb(r[i], plo, borrow);
        r[i] = diff;
        borrow = b_out;
        if i >= la && carry_hi == 0 && borrow == 0 {
            break;
        }
    }
    borrow != 0 || carry_hi != 0
}

fn div_rem_1_into(u: &[u64], d: u64, q_out: &mut LimbBuffer, r_out: &mut LimbBuffer, budget: &ExecutionBudget) -> Result<()> {
    assert_ne!(d, 0);
    let la = effective_len(u);
    if la == 1 && u[0] < d {
        q_out.set_zero(budget)?;
        r_out.copy_canonical(u, budget)?;
        return Ok(());
    }
    let q_storage = q_out.storage_mut(la, budget)?;
    q_storage.fill(0);
    let mut rem: u128 = 0;
    for i in (0..la).rev() {
        rem = (rem << 64) | u128::from(u[i]);
        let qi = rem / u128::from(d);
        rem %= u128::from(d);
        q_storage[i] = qi as u64;
    }
    q_out.trim_canonical();
    r_out.copy_canonical(&[rem as u64], budget)?;
    Ok(())
}

fn shl_into(v: &[u64], bits: u32, out: &mut [u64]) -> usize {
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

fn shr_into(v: &[u64], bits: u32, out: &mut [u64]) -> usize {
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

fn div_rem_knuth_into(
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

        while u128::from(qhat) >= (1u128 << 64)
            || (u128::from(qhat) * u128::from(v_n2) > (rhat << 64) + u128::from(u_work[j + n - 2]))
        {
            qhat = qhat.wrapping_sub(1);
            rhat += u128::from(v_n1);
            if rhat >= (1u128 << 64) {
                break;
            }
        }

        let borrow = submul_1_inplace(&mut u_work[j..j + n + 1], v_work, qhat);
        if borrow {
            qhat = qhat.wrapping_sub(1);
            let _ = addmul_1_inplace(&mut u_work[j..j + n + 1], v_work, 1);
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

// ——— 薄包装（内部 convenience / 旧调用方；默认 unlimited）———

/// 便利：分配新 `Vec` 的加法（**非热路径**；值层请用 `*_into` / executor）。
pub(crate) fn add_n_budgeted(a: &[u64], b: &[u64], budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::with_capacity(a.len().max(b.len()) + 1, budget)?;
        PureRustLimbKernel::add_into(a, b, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn add_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    add_n_budgeted(a, b, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn sub_n_budgeted(a: &[u64], b: &[u64], budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::with_capacity(a.len(), budget)?;
        PureRustLimbKernel::sub_into(a, b, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn sub_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    sub_n_budgeted(a, b, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn mul_budgeted(a: &[u64], b: &[u64], budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::zero();
        PureRustLimbKernel::mul_into(a, b, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    mul_budgeted(a, b, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn mul_1_budgeted(a: &[u64], n: u64, budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::zero();
        PureRustLimbKernel::mul_1_into(a, n, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn mul_1(a: &[u64], n: u64) -> Vec<u64> {
    mul_1_budgeted(a, n, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn sqr_budgeted(a: &[u64], budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::zero();
        PureRustLimbKernel::sqr_into(a, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn sqr(a: &[u64]) -> Vec<u64> {
    sqr_budgeted(a, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn div_rem_budgeted(u: &[u64], v: &[u64], budget: &ExecutionBudget) -> Result<(Vec<u64>, Vec<u64>)> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut q = LimbBuffer::zero();
        let mut r = LimbBuffer::zero();
        PureRustLimbKernel::div_rem_into(u, v, &mut q, &mut r, scratch, budget)?;
        Ok((q.into_canonical_vec(), r.into_canonical_vec()))
    })
}

pub(crate) fn div_rem(u: &[u64], v: &[u64]) -> (Vec<u64>, Vec<u64>) {
    div_rem_budgeted(u, v, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn addmul_1(r: &[u64], a: &[u64], n: u64) -> Vec<u64> {
    assert_ne!(n, 0);
    if is_zero(a) {
        return normalize_trim(r.to_vec());
    }
    let la = effective_len(a);
    let lr = effective_len(r);
    let mut out = r.to_vec();
    out.resize(lr.max(la) + 1, 0);
    addmul_1_inplace(&mut out, a, n);
    normalize_trim(out)
}

pub(crate) fn mul_schoolbook(a: &[u64], b: &[u64]) -> Vec<u64> {
    let la = effective_len(a);
    let lb = effective_len(b);
    let mut out = vec![0u64; (la + lb).max(1)];
    mul_schoolbook_into(a, b, &mut out);
    normalize_trim(out)
}

pub(crate) fn sqr_schoolbook(a: &[u64]) -> Vec<u64> {
    let la = effective_len(a);
    let mut out = vec![0u64; (2 * la).max(1)];
    sqr_schoolbook_into(a, &mut out);
    normalize_trim(out)
}

pub(crate) fn karatsuba_mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    mul(a, b)
}

// ——— GCD / Montgomery（仍走薄包装；后续可再收口到 scratch）———

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

const MONTGOMERY_THRESHOLD: usize = 2;

pub(crate) fn mod_pow_montgomery_eligible(modulus: &[u64]) -> bool {
    !is_zero(modulus) && (modulus[0] & 1) == 1 && effective_len(modulus) >= MONTGOMERY_THRESHOLD
}

fn montgomery_nprime(m0: u64) -> u64 {
    debug_assert!(m0 % 2 == 1);
    let mut x = 1u64;
    for _ in 0..6 {
        x = x.wrapping_mul(2u64.wrapping_sub(m0.wrapping_mul(x)));
    }
    x.wrapping_neg()
}

fn montgomery_redc(t: &mut [u64], m: &[u64], n_prime: u64) -> Vec<u64> {
    let n = effective_len(m);
    for i in 0..n {
        let u = t[i].wrapping_mul(n_prime);
        addmul_1_inplace(&mut t[i..], m, u);
    }
    let mut r = t[n..2 * n].to_vec();
    if cmp_slice(&r, m) != Ordering::Less {
        r = sub_n(&r, m);
    }
    normalize_trim(r)
}

pub(crate) fn div2_mod(exp: &mut Vec<u64>) {
    let len = effective_len(exp);
    if len == 0 {
        return;
    }
    let mut carry = 0u64;
    for i in (0..len).rev() {
        let limb = exp[i];
        let new_carry = limb & 1;
        exp[i] = (limb >> 1) | (carry << 63);
        carry = new_carry;
    }
    if len > 1 && exp[len - 1] == 0 {
        exp.pop();
    }
}

pub(crate) fn mod_pow_montgomery(base: &[u64], exp: &[u64], modulus: &[u64]) -> Vec<u64> {
    let (n_prime, r2) = montgomery_precompute(modulus);
    mod_pow_montgomery_precomputed(base, exp, modulus, n_prime, &r2)
}

pub(crate) fn montgomery_precompute(modulus: &[u64]) -> (u64, Vec<u64>) {
    let n_prime = montgomery_nprime(modulus[0]);
    let n = effective_len(modulus);
    let mut r = vec![0u64; n + 1];
    r[n] = 1;
    let r_mod = div_rem(&r, modulus).1;
    let r2 = div_rem(&mul(&r_mod, &r_mod), modulus).1;
    (n_prime, r2)
}

pub(crate) fn mod_pow_montgomery_precomputed(
    base: &[u64],
    exp: &[u64],
    modulus: &[u64],
    n_prime: u64,
    r2_mod_m: &[u64],
) -> Vec<u64> {
    assert!(!is_zero(modulus));
    if is_one(modulus) {
        return vec![0];
    }
    if is_zero(exp) {
        return vec![1];
    }

    let (_, base_reduced) = div_rem(base, modulus);
    let mut acc = to_mont_with(&[1], modulus, r2_mod_m, n_prime);
    let mut b = to_mont_with(&base_reduced, modulus, r2_mod_m, n_prime);
    let mut e = normalize_trim(exp.to_vec());

    while !is_zero(&e) {
        if (e[0] & 1) == 1 {
            acc = mul_mod_mont_with(&acc, &b, modulus, n_prime);
        }
        b = mul_mod_mont_with(&b, &b, modulus, n_prime);
        div2_mod(&mut e);
    }
    from_mont_with(&acc, modulus, n_prime)
}

fn to_mont_with(a: &[u64], m: &[u64], r2_mod_m: &[u64], n_prime: u64) -> Vec<u64> {
    mul_mod_mont_with(a, r2_mod_m, m, n_prime)
}

fn from_mont_with(a: &[u64], m: &[u64], n_prime: u64) -> Vec<u64> {
    let n = effective_len(m);
    let mut t = vec![0u64; 2 * n];
    let copy = effective_len(a).min(n);
    t[..copy].copy_from_slice(&a[..copy]);
    montgomery_redc(&mut t, m, n_prime)
}

fn mul_mod_mont_with(a: &[u64], b: &[u64], m: &[u64], n_prime: u64) -> Vec<u64> {
    let n = effective_len(m);
    let prod = mul(a, b);
    let mut t = vec![0u64; 2 * n];
    let copy_len = effective_len(&prod).min(2 * n);
    t[..copy_len].copy_from_slice(&prod[..copy_len]);
    montgomery_redc(&mut t, m, n_prime)
}

pub(crate) fn mul_mod_montgomery_precomputed(a: &[u64], b: &[u64], modulus: &[u64], n_prime: u64) -> Vec<u64> {
    mul_mod_mont_with(a, b, modulus, n_prime)
}

pub(crate) fn binary_gcd(mut a: Vec<u64>, mut b: Vec<u64>) -> Vec<u64> {
    a = normalize_trim(a);
    b = normalize_trim(b);
    if is_zero(&a) {
        return b;
    }
    if is_zero(&b) {
        return a;
    }

    let shift = trailing_zeros(&a).min(trailing_zeros(&b));
    shr_assign(&mut a, shift);
    shr_assign(&mut b, shift);

    loop {
        shr_assign_until_odd(&mut a);
        shr_assign_until_odd(&mut b);
        if cmp_slice(&a, &b) == Ordering::Equal {
            break;
        }
        if cmp_slice(&a, &b) == Ordering::Less {
            std::mem::swap(&mut a, &mut b);
        }
        if is_one(&b) {
            break;
        }
        a = sub_n(&a, &b);
        shr_assign(&mut a, 1);
    }
    a = b;
    shl_assign(&mut a, shift);
    normalize_trim(a)
}

fn is_one(v: &[u64]) -> bool {
    effective_len(v) == 1 && v[0] == 1
}

fn trailing_zeros(v: &[u64]) -> u32 {
    for (i, &limb) in v.iter().enumerate() {
        if limb != 0 {
            return i as u32 * 64 + limb.trailing_zeros();
        }
    }
    u32::MAX
}

fn shr_assign(v: &mut [u64], bits: u32) {
    if bits == 0 {
        return;
    }
    if bits >= 64 * v.len() as u32 {
        v.fill(0);
        v[0] = 0;
        return;
    }
    let whole = (bits / 64) as usize;
    let rem = bits % 64;
    if whole > 0 {
        v.copy_within(whole.., 0);
        let len = v.len();
        for x in &mut v[len - whole..] {
            *x = 0;
        }
    }
    if rem > 0 {
        let mut carry = 0u64;
        for i in (0..v.len()).rev() {
            let new_carry = v[i] << (64 - rem);
            v[i] = (v[i] >> rem) | carry;
            carry = new_carry;
        }
    }
}

fn shr_assign_until_odd(v: &mut [u64]) {
    let tz = trailing_zeros(v);
    if tz < u32::MAX {
        shr_assign(v, tz);
    }
}

fn shl_assign(v: &mut Vec<u64>, bits: u32) {
    if bits == 0 || is_zero(v) {
        return;
    }
    let extra = (bits / 64) as usize;
    if extra > 0 {
        v.splice(0..0, std::iter::repeat_n(0, extra));
    }
    let rem = bits % 64;
    if rem > 0 {
        let mut carry = 0u64;
        for limb in v.iter_mut() {
            let new_carry = *limb >> (64 - rem);
            *limb = (*limb << rem) | carry;
            carry = new_carry;
        }
        if carry != 0 {
            v.push(carry);
        }
    }
}

pub(crate) fn shr_natural(v: &[u64], bits: u32) -> (Vec<u64>, u64) {
    if bits == 0 || is_zero(v) {
        return (normalize_trim(v.to_vec()), 0);
    }
    if bits >= 64 * v.len() as u32 {
        return (vec![0], 0);
    }
    let whole = (bits / 64) as usize;
    let rem = bits % 64;
    let el = effective_len(v);
    let mut out = v[..el].to_vec();
    if whole > 0 {
        out.drain(0..whole);
        if out.is_empty() {
            out.push(0);
        }
    }
    let mut remainder = 0u64;
    if rem > 0 {
        let mut carry = 0u128;
        for i in (0..out.len()).rev() {
            let wide = u128::from(out[i]) | (carry << 64);
            out[i] = (wide >> rem) as u64;
            carry = wide & ((1u128 << rem) - 1);
        }
        remainder = carry as u64;
    }
    (normalize_trim(out), remainder)
}

/// 私有 limb 执行合同（输出缓冲 + scratch + 预算）。
pub(crate) trait LimbKernel {
    fn add_into(
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()>;

    fn sub_into(
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()>;

    fn mul_into(
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()>;

    fn mul_1_into(
        a: &[u64],
        limb: u64,
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()>;

    fn sqr_into(a: &[u64], out: &mut LimbBuffer, scratch: &mut ScratchWorkspace, budget: &ExecutionBudget) -> Result<()>;

    fn div_rem_into(
        u: &[u64],
        v: &[u64],
        q_out: &mut LimbBuffer,
        r_out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()>;
}

pub(crate) struct PureRustLimbKernel;

impl LimbKernel for PureRustLimbKernel {
    fn add_into(
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        _scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        let la = effective_len(a);
        let lb = effective_len(b);
        budget.check_add(la, lb)?;
        let n = la.max(lb);
        let storage = out.storage_mut(n + 1, budget)?;
        storage.fill(0);
        let mut carry = 0u64;
        for i in 0..n {
            let (sum, c) = adc(*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0), carry);
            storage[i] = sum;
            carry = c;
        }
        storage[n] = carry;
        out.trim_canonical();
        Ok(())
    }

    fn sub_into(
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        _scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        if cmp_slice(a, b) == Ordering::Less {
            return Err(kernel_err("sub_underflow"));
        }
        let n = effective_len(a);
        budget.check_limbs(n)?;
        let storage = out.storage_mut(n, budget)?;
        storage.fill(0);
        let mut borrow = 0u64;
        for i in 0..n {
            let (diff, b_out) = sbb(*a.get(i).unwrap_or(&0), *b.get(i).unwrap_or(&0), borrow);
            storage[i] = diff;
            borrow = b_out;
        }
        out.trim_canonical();
        Ok(())
    }

    fn mul_into(
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        use crate::{
            algorithm::{MulStrategy, select_mul_strategy},
            dispatch::AlgorithmCapability,
        };

        if is_zero(a) || is_zero(b) {
            return out.set_zero(budget);
        }
        let la = effective_len(a);
        let lb = effective_len(b);
        budget.check_mul(la, lb)?;
        match select_mul_strategy(la, lb, AlgorithmCapability::DEFAULT) {
            MulStrategy::Zero => out.set_zero(budget),
            MulStrategy::Schoolbook => {
                let storage = out.storage_mut(la + lb, budget)?;
                storage.fill(0);
                mul_schoolbook_into(a, b, storage);
                out.trim_canonical();
                Ok(())
            }
            MulStrategy::Karatsuba => {
                let scratch_need = karatsuba_scratch_limbs(la.max(lb));
                budget.check_limbs(scratch_need.max(la + lb))?;
                scratch.ensure(scratch_need, budget)?;
                let out_len = la + lb;
                let storage = out.storage_mut(out_len, budget)?;
                storage.fill(0);
                let scratch_slice = scratch.as_mut_slice();
                mul_rec(a, b, storage, scratch_slice);
                out.trim_canonical();
                Ok(())
            }
            MulStrategy::Toom3 => {
                let scratch_need = toom3_scratch_limbs(la.max(lb));
                budget.check_limbs(scratch_need.max(la + lb))?;
                scratch.ensure(scratch_need, budget)?;
                let out_len = la + lb;
                let storage = out.storage_mut(out_len, budget)?;
                storage.fill(0);
                let scratch_slice = scratch.as_mut_slice();
                toom3_mul_rec(a, b, storage, scratch_slice);
                out.trim_canonical();
                Ok(())
            }
        }
    }

    fn mul_1_into(
        a: &[u64],
        limb: u64,
        out: &mut LimbBuffer,
        _scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        if limb == 0 || is_zero(a) {
            return out.set_zero(budget);
        }
        let la = effective_len(a);
        budget.check_mul(la, 1)?;
        let storage = out.storage_mut(la + 1, budget)?;
        mul_1_into_slice(a, limb, storage);
        out.trim_canonical();
        Ok(())
    }

    fn sqr_into(a: &[u64], out: &mut LimbBuffer, scratch: &mut ScratchWorkspace, budget: &ExecutionBudget) -> Result<()> {
        if is_zero(a) {
            return out.set_zero(budget);
        }
        let la = effective_len(a);
        budget.check_mul(la, la)?;
        if la < MUL_KARATSUBA_THRESHOLD {
            let storage = out.storage_mut(2 * la, budget)?;
            sqr_schoolbook_into(a, storage);
            out.trim_canonical();
            return Ok(());
        }
        Self::mul_into(a, a, out, scratch, budget)
    }

    fn div_rem_into(
        u: &[u64],
        v: &[u64],
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
            return div_rem_1_into(u, v[0], q_out, r_out, budget);
        }
        use crate::{
            algorithm::{DivStrategy, select_div_strategy},
            dispatch::AlgorithmCapability,
        };
        match select_div_strategy(u_el, v_el, AlgorithmCapability::DEFAULT) {
            DivStrategy::Knuth => div_rem_knuth_into(u, v, q_out, r_out, scratch, budget),
            DivStrategy::BurnikelZiegler => div_rem_bz_into(u, v, q_out, r_out, scratch, budget),
        }
    }
}
