//! GCD 与二元路径使用的原地 / natural 左右移。

use super::primitive::{effective_len, is_zero, normalize_trim, trailing_zeros};

pub(super) fn shr_assign(v: &mut [u64], bits: u32) {
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

pub(super) fn shr_assign_until_odd(v: &mut [u64]) {
    let tz = trailing_zeros(v);
    if tz < u32::MAX {
        shr_assign(v, tz);
    }
}

pub(super) fn shl_assign(v: &mut Vec<u64>, bits: u32) {
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
