//! Limb kernel for the pure-Rust backend.
//!
//! All multi-precision primitives operate on little-endian canonical limb slices:
//! no trailing zero limbs except the single `[0]` zero value.

use athena_types::Result;

use crate::execution_budget::ExecutionBudget;

use super::buffer::{LimbBuffer, ScratchWorkspace, kernel_err};

use std::cell::RefCell;
use std::cmp::Ordering;

thread_local! {
    static KERNEL_SCRATCH: RefCell<ScratchWorkspace> = RefCell::new(ScratchWorkspace::default());
}

fn with_kernel_scratch<R>(f: impl FnOnce(&mut ScratchWorkspace, &ExecutionBudget) -> R) -> R {
    KERNEL_SCRATCH.with(|cell| {
        if let Ok(mut scratch) = cell.try_borrow_mut() {
            let budget = ExecutionBudget::unlimited();
            let result = f(&mut *scratch, &budget);
            scratch.clear();
            result
        }
        else {
            let mut scratch = ScratchWorkspace::default();
            let budget = ExecutionBudget::unlimited();
            f(&mut scratch, &budget)
        }
    })
}

/// Karatsuba multiplication threshold (limbs per operand).
pub(crate) const MUL_KARATSUBA_THRESHOLD: usize = 32;

/// Full-width single-limb product: `(hi, lo) = a * b`.
#[inline]
#[allow(dead_code)]
pub(crate) fn mul_wide(a: u64, b: u64) -> (u64, u64) {
    let prod = (a as u128) * (b as u128);
    ((prod >> 64) as u64, prod as u64)
}

/// Add with carry: `(sum, carry_out) = a + b + carry_in` (`carry_in` is 0 or 1).
#[inline]
pub(crate) fn adc(a: u64, b: u64, carry: u64) -> (u64, u64) {
    let sum = (a as u128) + (b as u128) + (carry as u128);
    (sum as u64, (sum >> 64) as u64)
}

/// Subtract with borrow: `(diff, borrow_out) = a - b - borrow_in`.
#[inline]
pub(crate) fn sbb(a: u64, b: u64, borrow: u64) -> (u64, u64) {
    let sub = (b as u128) + (borrow as u128);
    let a128 = a as u128;
    if a128 >= sub { ((a128 - sub) as u64, 0) } else { ((a128 + (1u128 << 64) - sub) as u64, 1) }
}

/// Fused multiply-add into limb: `(limb, carry) = acc + a * b + carry`.
#[inline]
pub(crate) fn mac(acc: u64, a: u64, b: u64, carry: u128) -> (u64, u128) {
    let sum = (acc as u128) + (a as u128) * (b as u128) + carry;
    (sum as u64, sum >> 64)
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

pub(crate) fn add_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    with_kernel_scratch(|scratch, budget| {
        let mut out = LimbBuffer::with_capacity(a.len().max(b.len()) + 1, budget).expect("unlimited");
        PureRustLimbKernel::add_into(a, b, &mut out, scratch, budget).expect("add_into");
        out.into_canonical_vec()
    })
}

pub(crate) fn sub_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    with_kernel_scratch(|scratch, budget| {
        let mut out = LimbBuffer::with_capacity(a.len(), budget).expect("unlimited");
        PureRustLimbKernel::sub_into(a, b, &mut out, scratch, budget).expect("sub_into");
        out.into_canonical_vec()
    })
}

pub(crate) fn mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    with_kernel_scratch(|scratch, budget| {
        let mut out = LimbBuffer::zero();
        PureRustLimbKernel::mul_into(a, b, &mut out, scratch, budget).expect("mul_into");
        out.into_canonical_vec()
    })
}

/// Multiply by a single limb (`n > 0`).
pub(crate) fn mul_1(a: &[u64], n: u64) -> Vec<u64> {
    with_kernel_scratch(|scratch, budget| {
        let mut out = LimbBuffer::zero();
        PureRustLimbKernel::mul_1_into(a, n, &mut out, scratch, budget).expect("mul_1_into");
        out.into_canonical_vec()
    })
}

fn mul_1_limbs(a: &[u64], limb: u64) -> Vec<u64> {
    if limb == 0 || is_zero(a) {
        return vec![0];
    }
    if limb == 1 {
        return normalize_trim(a.to_vec());
    }
    let la = effective_len(a);
    let mut out = vec![0u64; la + 1];
    let mut carry = 0u128;
    for i in 0..la {
        let prod = u128::from(a[i]) * u128::from(limb) + carry;
        out[i] = prod as u64;
        carry = prod >> 64;
    }
    if carry > 0 {
        out[la] = carry as u64;
    }
    normalize_trim(out)
}

/// In-place `r += a * n` for single limb `n > 0`.
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

/// Subtract `a * n` from `r`; returns `true` on borrow (underflow).
pub(crate) fn submul_1_inplace(r: &mut [u64], a: &[u64], n: u64) -> bool {
    if n == 0 || is_zero(a) {
        return false;
    }
    let prod = mul_1_limbs(a, n);
    let mut borrow = 0i128;
    for (i, &pv) in prod.iter().enumerate() {
        if i >= r.len() {
            return true;
        }
        let ri = r[i] as i128;
        let sub = pv as i128 + borrow;
        if ri >= sub {
            r[i] = (ri - sub) as u64;
            borrow = 0;
        }
        else {
            r[i] = (ri + (1i128 << 64) - sub) as u64;
            borrow = 1;
        }
    }
    if borrow == 0 {
        return false;
    }
    let mut idx = prod.len();
    while borrow != 0 {
        if idx >= r.len() {
            return true;
        }
        if r[idx] > 0 {
            r[idx] -= 1;
            borrow = 0;
        }
        else {
            r[idx] = u64::MAX;
            idx += 1;
        }
    }
    false
}

/// Fused add-multiply: `r + a * n` for single limb `n > 0`.
pub(crate) fn addmul_1(r: &[u64], a: &[u64], n: u64) -> Vec<u64> {
    assert!(n != 0);
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

fn add_wide_to_out(out: &mut Vec<u64>, idx: usize, wide: u128) {
    let mut carry = wide;
    let mut k = idx;
    while carry > 0 {
        if k >= out.len() {
            out.push(0);
        }
        let sum = u128::from(out[k]) + carry;
        out[k] = sum as u64;
        carry = sum >> 64;
        k += 1;
    }
}

/// Symmetric schoolbook squaring: diagonal once, off-diagonal doubled (`j > i` only).
fn sqr_schoolbook(a: &[u64]) -> Vec<u64> {
    let la = effective_len(a);
    if la == 0 || is_zero(a) {
        return vec![0];
    }
    if la == 1 {
        return mul_1_limbs(a, a[0]);
    }
    let mut out = vec![0u64; 2 * la];
    for i in 0..la {
        for j in i..la {
            let (hi, lo) = mul_wide(a[i], a[j]);
            let prod = (u128::from(hi) << 64) | u128::from(lo);
            let idx = i + j;
            if i == j {
                add_wide_to_out(&mut out, idx, prod);
            }
            else {
                add_wide_to_out(&mut out, idx, prod);
                add_wide_to_out(&mut out, idx, prod);
            }
        }
    }
    normalize_trim(out)
}

/// Square (`a * a`).
pub(crate) fn sqr(a: &[u64]) -> Vec<u64> {
    with_kernel_scratch(|scratch, budget| {
        let mut out = LimbBuffer::zero();
        PureRustLimbKernel::sqr_into(a, &mut out, scratch, budget).expect("sqr_into");
        out.into_canonical_vec()
    })
}

fn mul_schoolbook(a: &[u64], b: &[u64]) -> Vec<u64> {
    let la = effective_len(a);
    let lb = effective_len(b);
    let mut out = vec![0u64; la + lb];
    for (i, &av) in a.iter().take(la).enumerate() {
        let mut carry = 0u128;
        for (j, &bv) in b.iter().take(lb).enumerate() {
            let idx = i + j;
            let (limb, c) = mac(out[idx], av, bv, carry);
            out[idx] = limb;
            carry = c;
        }
        let mut k = i + lb;
        while carry > 0 {
            if k >= out.len() {
                out.push(0);
            }
            let sum = (out[k] as u128) + carry;
            out[k] = sum as u64;
            carry = sum >> 64;
            k += 1;
        }
    }
    normalize_trim(out)
}

fn karatsuba_mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    let la = effective_len(a);
    let lb = effective_len(b);
    let n = la.max(lb).next_power_of_two().max(MUL_KARATSUBA_THRESHOLD);
    let ah = split_hi(a, n);
    let al = split_lo(a, n);
    let bh = split_hi(b, n);
    let bl = split_lo(b, n);
    let z0 = mul(&al, &bl);
    let z2 = mul(&ah, &bh);
    let a_sum = add_n(&al, &ah);
    let b_sum = add_n(&bl, &bh);
    let z1 = sub_n(&sub_n(&mul(&a_sum, &b_sum), &z0), &z2);
    let m = n / 2;
    let mid = shift_limbs_left(z1, m);
    let high = shift_limbs_left(z2, n);
    add_n(&add_n(&z0, &mid), &high)
}

fn split_lo(v: &[u64], n: usize) -> Vec<u64> {
    let mut out = v.iter().take(n / 2).copied().collect::<Vec<_>>();
    if out.is_empty() {
        out.push(0);
    }
    normalize_trim(out)
}

fn split_hi(v: &[u64], n: usize) -> Vec<u64> {
    let mut out = v.iter().skip(n / 2).copied().collect::<Vec<_>>();
    if out.is_empty() {
        out.push(0);
    }
    normalize_trim(out)
}

fn shift_limbs_left(v: Vec<u64>, limbs: usize) -> Vec<u64> {
    if is_zero(&v) || limbs == 0 {
        return v;
    }
    let mut out = vec![0u64; limbs];
    out.extend(v);
    normalize_trim(out)
}

pub(crate) fn div_rem(u: &[u64], v: &[u64]) -> (Vec<u64>, Vec<u64>) {
    with_kernel_scratch(|scratch, budget| {
        let mut q = LimbBuffer::zero();
        let mut r = LimbBuffer::zero();
        PureRustLimbKernel::div_rem_into(u, v, &mut q, &mut r, scratch, budget).expect("div_rem_into");
        (q.into_canonical_vec(), r.into_canonical_vec())
    })
}

fn div_rem_1(u: Vec<u64>, d: u64) -> (Vec<u64>, Vec<u64>) {
    assert!(d != 0);
    let la = effective_len(&u);
    if la == 1 && u[0] < d {
        return (vec![0], u);
    }
    let mut q = vec![0u64; la];
    let mut rem: u128 = 0;
    for i in (0..la).rev() {
        rem = (rem << 64) | u128::from(u[i]);
        let qi = rem / u128::from(d);
        rem %= u128::from(d);
        q[i] = qi as u64;
        let mut carry = qi >> 64;
        let mut j = i + 1;
        while carry > 0 {
            if j >= q.len() {
                q.push(0);
            }
            let sum = u128::from(q[j]) + carry;
            q[j] = sum as u64;
            carry = sum >> 64;
            j += 1;
        }
    }
    (normalize_trim(q), vec![rem as u64])
}

fn div_rem_knuth(mut u: Vec<u64>, v: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let n = effective_len(v);
    assert!(n >= 2);
    let m = u.len().checked_sub(n).unwrap_or(0);
    let shift = v[n - 1].leading_zeros();
    if shift > 0 {
        u = shl_vec(&u, shift);
    }
    let v = if shift > 0 { shl_vec(v, shift) } else { v.to_vec() };
    u.resize(m + n + 1, 0);

    let v_n1 = v[n - 1];
    let v_n2 = v[n - 2];
    let mut q = vec![0u64; m + 1];

    for j in (0..=m).rev() {
        let u_jn = u[j + n];
        let u_jn1 = u[j + n - 1];
        let top = (u128::from(u_jn) << 64) | u128::from(u_jn1);
        let mut qhat = if u_jn >= v_n1 { u64::MAX } else { (top / u128::from(v_n1)) as u64 };
        let mut rhat = top - u128::from(qhat) * u128::from(v_n1);

        while u128::from(qhat) >= (1u128 << 64)
            || (u128::from(qhat) * u128::from(v_n2) > (rhat << 64) + u128::from(u[j + n - 2]))
        {
            qhat -= 1;
            rhat += u128::from(v_n1);
            if rhat >= (1u128 << 64) {
                break;
            }
        }

        let borrow = submul_1_inplace(&mut u[j..j + n + 1], &v, qhat);

        if borrow {
            qhat -= 1;
            addmul_1_inplace(&mut u[j..j + n + 1], &v, 1);
        }
        q[j] = qhat;
    }

    let mut r = u.into_iter().take(n).collect::<Vec<_>>();
    if shift > 0 {
        r = shr_vec(&r, shift);
    }
    (normalize_trim(q), normalize_trim(r))
}

fn shl_vec(v: &[u64], bits: u32) -> Vec<u64> {
    if bits == 0 || is_zero(v) {
        return v.to_vec();
    }
    if bits == 64 {
        let mut out = vec![0];
        out.extend_from_slice(v);
        return normalize_trim(out);
    }
    let mut out = vec![0u64; v.len() + 1];
    let mut carry = 0u64;
    for (i, &limb) in v.iter().enumerate() {
        out[i] = (limb << bits) | carry;
        carry = limb >> (64 - bits);
    }
    if carry != 0 {
        out[v.len()] = carry;
    }
    normalize_trim(out)
}

fn shr_vec(v: &[u64], bits: u32) -> Vec<u64> {
    if bits == 0 || is_zero(v) {
        return v.to_vec();
    }
    let mut out = vec![0u64; v.len()];
    let mut carry = 0u128;
    for i in (0..v.len()).rev() {
        let wide = u128::from(v[i]) | (carry << 64);
        out[i] = (wide >> bits) as u64;
        carry = wide & ((1u128 << bits) - 1);
    }
    normalize_trim(out)
}

const LEHMER_THRESHOLD: usize = 3;

/// Non-negative GCD (Lehmer for wide operands, binary GCD tail).
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

    let Some(na_new) = lincomb_signed(x0, a, x1, b) else {
        return false;
    };
    let Some(nb_new) = lincomb_signed(y0, a, y1, b) else {
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

/// Odd modulus with at least two limbs → Montgomery mod_pow eligible.
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

fn to_mont(a: &[u64], m: &[u64], r2_mod_m: &[u64]) -> Vec<u64> {
    mul_mod_mont(a, r2_mod_m, m)
}

fn from_mont(a: &[u64], m: &[u64]) -> Vec<u64> {
    let n = effective_len(m);
    let n_prime = montgomery_nprime(m[0]);
    let mut t = vec![0u64; 2 * n];
    let copy = effective_len(a).min(n);
    t[..copy].copy_from_slice(&a[..copy]);
    montgomery_redc(&mut t, m, n_prime)
}

fn mul_mod_mont(a: &[u64], b: &[u64], m: &[u64]) -> Vec<u64> {
    let n = effective_len(m);
    let n_prime = montgomery_nprime(m[0]);
    let prod = mul(a, b);
    let mut t = vec![0u64; 2 * n];
    let copy_len = effective_len(&prod).min(2 * n);
    t[..copy_len].copy_from_slice(&prod[..copy_len]);
    montgomery_redc(&mut t, m, n_prime)
}

fn div2_mod(exp: &mut Vec<u64>) {
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

/// Modular exponentiation via Montgomery reduction (odd `modulus`).
pub(crate) fn mod_pow_montgomery(base: &[u64], exp: &[u64], modulus: &[u64]) -> Vec<u64> {
    assert!(!is_zero(modulus));
    if is_one(modulus) {
        return vec![0];
    }
    if is_zero(exp) {
        return vec![1];
    }

    let r2 = {
        let n = effective_len(modulus);
        let mut r = vec![0u64; n + 1];
        r[n] = 1;
        let r_mod = div_rem(&r, modulus).1;
        div_rem(&mul(&r_mod, &r_mod), modulus).1
    };
    let (_, base_reduced) = div_rem(base, modulus);
    let mut acc = to_mont(&[1], modulus, &r2);
    let mut b = to_mont(&base_reduced, modulus, &r2);
    let mut e = normalize_trim(exp.to_vec());

    while !is_zero(&e) {
        if (e[0] & 1) == 1 {
            acc = mul_mod_mont(&acc, &b, modulus);
        }
        b = mul_mod_mont(&b, &b, modulus);
        div2_mod(&mut e);
    }
    from_mont(&acc, modulus)
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

/// Right-shift canonical limbs by `bits`, returning quotient limbs and low remainder bits.
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

/// Private limb execution contract (output buffers + scratch + budget).
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

    fn sqr_into(
        a: &[u64],
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()>;

    fn div_rem_into(
        u: &[u64],
        v: &[u64],
        q_out: &mut LimbBuffer,
        r_out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()>;
}

/// Default pure-Rust limb kernel.
pub(crate) struct PureRustLimbKernel;

impl LimbKernel for PureRustLimbKernel {
    fn add_into(
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        _scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        budget.check_add(effective_len(a), effective_len(b))?;
        let n = a.len().max(b.len());
        let storage = out.storage_mut(n + 1, budget)?;
        let mut carry = 0u64;
        for i in 0..n {
            let av = *a.get(i).unwrap_or(&0);
            let bv = *b.get(i).unwrap_or(&0);
            let (sum, c) = adc(av, bv, carry);
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
        budget.check_limbs(effective_len(a))?;
        let n = a.len();
        out.set_canonical(a.to_vec(), budget)?;
        let storage = out.storage_mut(n, budget)?;
        let mut borrow = 0u64;
        for i in 0..n {
            let (diff, b_out) = sbb(storage[i], *b.get(i).unwrap_or(&0), borrow);
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
        _scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        if is_zero(a) || is_zero(b) {
            out.set_canonical(vec![0], budget)?;
            return Ok(());
        }
        let la = effective_len(a);
        let lb = effective_len(b);
        budget.check_mul(la, lb)?;
        budget.check_mul_scratch(la, lb)?;
        let product = if la.max(lb) >= MUL_KARATSUBA_THRESHOLD { karatsuba_mul(a, b) } else { mul_schoolbook(a, b) };
        out.set_canonical(product, budget)?;
        Ok(())
    }

    fn mul_1_into(
        a: &[u64],
        limb: u64,
        out: &mut LimbBuffer,
        _scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        if limb == 0 || is_zero(a) {
            out.set_canonical(vec![0], budget)?;
            return Ok(());
        }
        budget.check_mul(effective_len(a), 1)?;
        out.set_canonical(mul_1_limbs(a, limb), budget)?;
        Ok(())
    }

    fn sqr_into(
        a: &[u64],
        out: &mut LimbBuffer,
        _scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        if is_zero(a) {
            out.set_canonical(vec![0], budget)?;
            return Ok(());
        }
        let la = effective_len(a);
        budget.check_mul(la, la)?;
        let product = if la >= MUL_KARATSUBA_THRESHOLD { karatsuba_mul(a, a) } else { sqr_schoolbook(a) };
        out.set_canonical(product, budget)?;
        Ok(())
    }

    fn div_rem_into(
        u: &[u64],
        v: &[u64],
        q_out: &mut LimbBuffer,
        r_out: &mut LimbBuffer,
        _scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        let u_norm = normalize_trim(u.to_vec());
        let v_norm = normalize_trim(v.to_vec());
        if is_zero(&v_norm) {
            return Err(kernel_err("div_zero"));
        }
        budget.check_div(effective_len(&u_norm), effective_len(&v_norm))?;
        let (q, r) = if is_zero(&u_norm) || cmp_slice(&u_norm, &v_norm) == Ordering::Less {
            (vec![0], u_norm)
        }
        else if effective_len(&v_norm) == 1 {
            div_rem_1(u_norm, v_norm[0])
        }
        else {
            div_rem_knuth(u_norm, &v_norm)
        };
        q_out.set_canonical(q, budget)?;
        r_out.set_canonical(r, budget)?;
        Ok(())
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn limb_kernel_add_into_respects_budget() {
        let budget = ExecutionBudget::from_limits(&crate::backends::NumericBackendLimits {
            max_limbs: Some(2),
            max_significand_bits: None,
            max_wire_payload_bytes: None,
            max_pow_exp: None,
        });
        let a = vec![1u64, 1u64, 1u64];
        let b = vec![1u64];
        let mut out = LimbBuffer::zero();
        let mut scratch = ScratchWorkspace::default();
        let err = PureRustLimbKernel::add_into(&a, &b, &mut out, &mut scratch, &budget).unwrap_err();
        assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_RESOURCE_LIMIT");
    }
}

#[cfg(test)]
mod primitive_tests {
    use super::*;

    fn lcg_next(state: &mut u64) -> u64 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *state
    }

    #[test]
    fn mul_wide_matches_u128_product() {
        for &a in &[0, 1, u64::MAX - 1, u64::MAX] {
            for &b in &[0, 1, u64::MAX - 1, u64::MAX] {
                let (hi, lo) = mul_wide(a, b);
                let prod = (a as u128) * (b as u128);
                assert_eq!(lo, prod as u64);
                assert_eq!(hi, (prod >> 64) as u64);
            }
        }
        let mut seed = 0xC0FFEE_u64;
        for _ in 0..50_000 {
            let a = lcg_next(&mut seed);
            let b = lcg_next(&mut seed);
            let (hi, lo) = mul_wide(a, b);
            let prod = (a as u128) * (b as u128);
            assert_eq!(lo, prod as u64);
            assert_eq!(hi, (prod >> 64) as u64);
        }
    }

    #[test]
    fn adc_matches_u128_add_with_carry() {
        for carry in 0u64..=1 {
            for &a in &[0, 1, u64::MAX - 1, u64::MAX] {
                for &b in &[0, 1, u64::MAX - 1, u64::MAX] {
                    let (sum, c_out) = adc(a, b, carry);
                    let wide = (a as u128) + (b as u128) + (carry as u128);
                    assert_eq!(sum, wide as u64);
                    assert_eq!(c_out, (wide >> 64) as u64);
                }
            }
        }
        let mut seed = 0xADC_u64;
        for _ in 0..50_000 {
            let a = lcg_next(&mut seed);
            let b = lcg_next(&mut seed);
            let carry = lcg_next(&mut seed) & 1;
            let (sum, c_out) = adc(a, b, carry);
            let wide = (a as u128) + (b as u128) + (carry as u128);
            assert_eq!(sum, wide as u64);
            assert_eq!(c_out, (wide >> 64) as u64);
        }
    }

    #[test]
    fn sbb_matches_borrow_subtraction() {
        for borrow in 0u64..=1 {
            for &a in &[0, 1, u64::MAX - 1, u64::MAX] {
                for &b in &[0, 1, u64::MAX - 1, u64::MAX] {
                    let (diff, b_out) = sbb(a, b, borrow);
                    let sub = (b as u128) + (borrow as u128);
                    let a128 = a as u128;
                    let (ref_diff, ref_borrow) =
                        if a128 >= sub { ((a128 - sub) as u64, 0) } else { ((a128 + (1u128 << 64) - sub) as u64, 1) };
                    assert_eq!(diff, ref_diff);
                    assert_eq!(b_out, ref_borrow);
                }
            }
        }
        let mut seed = 0x5BB_u64;
        for _ in 0..50_000 {
            let a = lcg_next(&mut seed);
            let b = lcg_next(&mut seed);
            let borrow = lcg_next(&mut seed) & 1;
            let (diff, b_out) = sbb(a, b, borrow);
            let sub = (b as u128) + (borrow as u128);
            let a128 = a as u128;
            let (ref_diff, ref_borrow) =
                if a128 >= sub { ((a128 - sub) as u64, 0) } else { ((a128 + (1u128 << 64) - sub) as u64, 1) };
            assert_eq!(diff, ref_diff);
            assert_eq!(b_out, ref_borrow);
        }
    }

    #[test]
    fn mac_matches_fused_multiply_add() {
        for &acc in &[0, u64::MAX] {
            for &a in &[0, 1, u64::MAX] {
                for &b in &[0, 1, u64::MAX] {
                    for carry in [0u128, 1, u64::MAX as u128] {
                        let (limb, c_out) = mac(acc, a, b, carry);
                        let wide = (acc as u128) + (a as u128) * (b as u128) + carry;
                        assert_eq!(limb, wide as u64);
                        assert_eq!(c_out, wide >> 64);
                    }
                }
            }
        }
        let mut seed = 0xA0C_u64;
        for _ in 0..50_000 {
            let acc = lcg_next(&mut seed);
            let a = lcg_next(&mut seed);
            let b = lcg_next(&mut seed);
            // Carry in schoolbook is always `sum >> 64` from the prior mac step.
            let carry = lcg_next(&mut seed) as u128;
            let (limb, c_out) = mac(acc, a, b, carry);
            let wide = (acc as u128) + (a as u128) * (b as u128) + carry;
            assert_eq!(limb, wide as u64);
            assert_eq!(c_out, wide >> 64);
        }
    }

    #[test]
    fn karatsuba_matches_schoolbook() {
        let mut seed = 0x4710_u64;
        for _ in 0..32 {
            let la = (lcg_next(&mut seed) as usize % 80) + MUL_KARATSUBA_THRESHOLD;
            let lb = (lcg_next(&mut seed) as usize % 80) + MUL_KARATSUBA_THRESHOLD;
            let a: Vec<u64> = (0..la).map(|_| lcg_next(&mut seed)).collect();
            let b: Vec<u64> = (0..lb).map(|_| lcg_next(&mut seed)).collect();
            let school = mul_schoolbook(&a, &b);
            let kara = karatsuba_mul(&a, &b);
            assert_eq!(school, kara, "Karatsuba diverged from schoolbook");
        }
    }

    #[test]
    fn sqr_matches_mul() {
        let mut seed = 0x5A00_u64;
        for _ in 0..64 {
            let la = (lcg_next(&mut seed) as usize % 80) + 1;
            let a: Vec<u64> = (0..la).map(|_| lcg_next(&mut seed)).collect();
            let via_mul = mul(&a, &a);
            let via_sqr = sqr(&a);
            assert_eq!(via_mul, via_sqr, "sqr diverged from mul");
        }
    }

    #[test]
    fn mul_1_matches_mul_single_limb() {
        let mut seed = 0xB160_u64;
        for _ in 0..256 {
            let la = (lcg_next(&mut seed) as usize % 40) + 1;
            let a: Vec<u64> = (0..la).map(|_| lcg_next(&mut seed)).collect();
            let n = lcg_next(&mut seed) | 1;
            assert_eq!(mul_1(&a, n), mul(&a, &[n]));
        }
    }

    #[test]
    fn addmul_1_matches_add_mul_1() {
        let mut seed = 0xC170_u64;
        for _ in 0..256 {
            let lr = (lcg_next(&mut seed) as usize % 40) + 1;
            let la = (lcg_next(&mut seed) as usize % 40) + 1;
            let r: Vec<u64> = (0..lr).map(|_| lcg_next(&mut seed)).collect();
            let a: Vec<u64> = (0..la).map(|_| lcg_next(&mut seed)).collect();
            let n = lcg_next(&mut seed) | 1;
            assert_eq!(addmul_1(&r, &a, n), add_n(&r, &mul_1(&a, n)));
        }
    }

    #[test]
    fn lehmer_gcd_matches_binary_gcd() {
        let mut seed = 0x6CD2_u64;
        for _ in 0..64 {
            let la = (lcg_next(&mut seed) as usize % 48) + 1;
            let lb = (lcg_next(&mut seed) as usize % 48) + 1;
            let a: Vec<u64> = (0..la).map(|_| lcg_next(&mut seed)).collect();
            let b: Vec<u64> = (0..lb).map(|_| lcg_next(&mut seed)).collect();
            assert_eq!(gcd(a.clone(), b.clone()), binary_gcd(a, b));
        }
    }

    #[test]
    fn montgomery_mod_pow_matches_binary() {
        let mut seed = 0xF002_u64;
        for _ in 0..32 {
            let base: Vec<u64> = (0..4).map(|_| lcg_next(&mut seed)).collect();
            let mut exp = vec![lcg_next(&mut seed) | 1, lcg_next(&mut seed)];
            if is_zero(&exp) {
                exp = vec![1];
            }
            let m = vec![0xFFFF_FFFF_FFFF_FFFDu64, 1];
            let via_mont = mod_pow_montgomery(&base, &exp, &m);
            let via_bin = std_mod_pow(&base, &exp, &m);
            assert_eq!(via_mont, via_bin);
        }
    }

    fn std_mod_pow(base: &[u64], exp: &[u64], modulus: &[u64]) -> Vec<u64> {
        let (_, mut base_r) = div_rem(base, modulus);
        let mut result = vec![1];
        let mut e = exp.to_vec();
        while !is_zero(&e) {
            if (e[0] & 1) == 1 {
                result = div_rem(&mul(&result, &base_r), modulus).1;
            }
            base_r = div_rem(&mul(&base_r, &base_r), modulus).1;
            div2_mod(&mut e);
        }
        normalize_trim(result)
    }
}
