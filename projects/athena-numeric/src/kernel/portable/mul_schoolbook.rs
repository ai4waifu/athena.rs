//! Schoolbook multiplication and single-limb fused mul-add/sub.

use super::primitive::{effective_len, is_zero, mac, mul_wide, sbb};

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

pub(super) fn mul_1_into_slice(a: &[u64], limb: u64, out: &mut [u64]) {
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

pub(super) fn sqr_schoolbook_into(a: &[u64], out: &mut [u64]) {
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
