//! Toom-3 (Bodrato) five-point multiplication.

use std::cmp::Ordering;

use crate::algorithm::MUL_TOOM_THRESHOLD;

use super::mul_karatsuba::mul_rec;
use super::mul_schoolbook::addmul_1_inplace;
use super::primitive::{adc, cmp_slice, effective_len, is_zero};
use super::slice_ops::{add_assign_shifted, sub_assign_slices};

/// Toom-3（Bodrato）：五点求值 `0,1,-1,2,∞` + 插值；子乘积走 `mul_rec`（无 `Vec`）。
/// Toom–3 multiplication by polynomial evaluation and interpolation.
///
/// Regard each operand as `A(X)=a₀+a₁X+a₂X²` with `X=βᵏ`. Five values determine
/// the degree-four product: evaluate at `0, 1, −1, 2, ∞`, multiply pointwise,
/// and interpolate. The `∞` value is the leading coefficient `a₂b₂`.
/// Interpolation divides by 2 and 3 exactly; a non-zero remainder is a bug.
/// This saves products asymptotically (Θ(nˡᵒᵍ³⁵)) but performs many signed
/// evaluations and temporaries, so the planner must reject small and badly
/// unbalanced inputs. `out` is zeroed, has `a.len()+b.len()` capacity, and
/// `scratch` must satisfy `toom3_scratch_limbs`.
pub(super) fn toom3_mul_rec(a: &[u64], b: &[u64], out: &mut [u64], scratch: &mut [u64]) {
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
