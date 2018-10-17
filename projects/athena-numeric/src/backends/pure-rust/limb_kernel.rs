//! Limb kernel for the pure-Rust backend.
//!
//! All multi-precision primitives operate on little-endian canonical limb slices:
//! no trailing zero limbs except the single `[0]` zero value.

use std::cmp::Ordering;

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
    let n = a.len().max(b.len());
    let mut out = Vec::with_capacity(n + 1);
    let mut carry = 0u64;
    for i in 0..n {
        let av = *a.get(i).unwrap_or(&0);
        let bv = *b.get(i).unwrap_or(&0);
        let (sum, c) = adc(av, bv, carry);
        out.push(sum);
        carry = c;
    }
    if carry > 0 {
        out.push(carry);
    }
    normalize_trim(out)
}

pub(crate) fn sub_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    assert!(cmp_slice(a, b) != Ordering::Less);
    let mut out = a.to_vec();
    let mut borrow = 0u64;
    for i in 0..out.len() {
        let (diff, b_out) = sbb(out[i], *b.get(i).unwrap_or(&0), borrow);
        out[i] = diff;
        borrow = b_out;
    }
    normalize_trim(out)
}

pub(crate) fn mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    if is_zero(a) || is_zero(b) {
        return vec![0];
    }
    let la = effective_len(a);
    let lb = effective_len(b);
    if la.max(lb) >= MUL_KARATSUBA_THRESHOLD { karatsuba_mul(a, b) } else { mul_schoolbook(a, b) }
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
    let u = normalize_trim(u.to_vec());
    let v = normalize_trim(v.to_vec());
    assert!(!is_zero(&v));
    if is_zero(&u) || cmp_slice(&u, &v) == Ordering::Less {
        return (vec![0], u);
    }
    if effective_len(&v) == 1 {
        return div_rem_1(u, v[0]);
    }
    div_rem_knuth(u, &v)
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

        let mut mul = vec![0u64; n + 1];
        let mut carry = 0u128;
        for i in 0..n {
            let prod = u128::from(qhat) * u128::from(v[i]) + carry;
            mul[i] = prod as u64;
            carry = prod >> 64;
        }
        mul[n] = carry as u64;

        let mut borrow = 0i128;
        for i in 0..=n {
            let ui = u[j + i] as i128 - borrow;
            let mi = mul[i] as i128;
            if ui >= mi {
                u[j + i] = (ui - mi) as u64;
                borrow = 0;
            }
            else {
                u[j + i] = (ui + (1i128 << 64) - mi) as u64;
                borrow = 1;
            }
        }

        if borrow != 0 {
            qhat -= 1;
            let mut carry = 0u128;
            for i in 0..n {
                let sum = u128::from(u[j + i]) + u128::from(v[i]) + carry;
                u[j + i] = sum as u64;
                carry = sum >> 64;
            }
            u[j + n] = u[j + n].wrapping_add(carry as u64);
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
                    let (ref_diff, ref_borrow) = if a128 >= sub {
                        ((a128 - sub) as u64, 0)
                    } else {
                        ((a128 + (1u128 << 64) - sub) as u64, 1)
                    };
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
            let (ref_diff, ref_borrow) = if a128 >= sub {
                ((a128 - sub) as u64, 0)
            } else {
                ((a128 + (1u128 << 64) - sub) as u64, 1)
            };
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
}
