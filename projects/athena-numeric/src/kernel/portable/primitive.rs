//! Portable limb primitives and fixed-width contracts.
//!
//! No algorithm strategy, no `Vec` hot-path ownership, no GC / budget / planner.

use std::cmp::Ordering;

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
    if hi == 0 { if mid == 0 { ([lo, 0, 0], if lo == 0 { 0 } else { 1 }) } else { ([lo, mid, 0], 2) } } else { ([lo, mid, hi], 3) }
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

pub(super) fn is_one(v: &[u64]) -> bool {
    effective_len(v) == 1 && v[0] == 1
}

pub(super) fn trailing_zeros(v: &[u64]) -> u32 {
    for (i, &limb) in v.iter().enumerate() {
        if limb != 0 {
            return i as u32 * 64 + limb.trailing_zeros();
        }
    }
    u32::MAX
}
