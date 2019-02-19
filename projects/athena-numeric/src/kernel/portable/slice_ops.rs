//! Shared slice helpers for divide-and-conquer multiplication.

use std::cmp::Ordering;

use super::primitive::{adc, cmp_slice, effective_len, is_zero, sbb};

pub(super) fn trim_slice_len(v: &mut [u64]) -> usize {
    let mut n = v.len();
    while n > 1 && v[n - 1] == 0 {
        n -= 1;
    }
    n
}

pub(super) fn split_lo_hi(v: &[u64], mid: usize) -> (&[u64], &[u64]) {
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

pub(super) fn add_slices_into(a: &[u64], b: &[u64], out: &mut [u64]) {
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

pub(super) fn sub_slices_into(a: &[u64], b: &[u64], out: &mut [u64]) {
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

pub(super) fn sub_assign_slices(a: &mut [u64], b: &[u64]) {
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

pub(super) fn add_assign_shifted(out: &mut [u64], src: &[u64], shift_limbs: usize) {
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
