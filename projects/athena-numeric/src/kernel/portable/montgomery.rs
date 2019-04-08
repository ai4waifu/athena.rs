//! # Purpose
//! Montgomery reduction (REDC), Montgomery multiplication, and modular exponentiation
//! for odd moduli.
//!
//! # Mathematical model
//! With $R = \beta^k > m$ and $m' \equiv -m^{-1} \pmod{\beta}$, REDC maps
//! $t$ to $t R^{-1} \bmod m$ using only shifts and multiply-adds. Working in
//! Montgomery form $\tilde a = a R \bmod m$ turns modular mul into REDC of a product.
//!
//! # Derivation
//! Choosing each $u_i = t_i m' \bmod \beta$ clears limb $i$ of $t + u_i m$.
//! After $k$ steps, a conditional subtract yields a residue $< m$.
//!
//! # Algorithm steps
//! 1. montgomery_nprime / montgomery_precompute (R^2 \bmod m).
//! 2. Convert in via mul by $R^2$ then REDC; multiply with REDC; convert out.
//! 3. mod_pow_montgomery_*: square-and-multiply in Montgomery domain.
//!
//! # Preconditions
//! - Odd modulus; width $\ge$ MONTGOMERY_THRESHOLD.
//! - Even moduli must not use this path (mod_pow_montgomery_eligible).
//!
//! # Postconditions
//! - Results are canonical residues in $[0,m)$.
//!
//! # Complexity
//! Exponentiation $O(\log e)$ Montgomery muls; each REDC is $O(k^2)$ limb work.
//!
//! # Crossover
//! Used when modulus is odd and wide enough; small/even moduli use generic paths.
//!
//! # Failure modes
//! Even $m$ has no inverse of $R$; eligible gate must hold.
//!
//! # Tests
//! Modular / mod_pow suites under `tests/exact/` and differential pure tests.

use std::cmp::Ordering;

use super::{
    convenience::{div_rem, mul, sub_n},
    mul_schoolbook::addmul_1_inplace,
    primitive::{cmp_slice, effective_len, is_one, is_zero, normalize_trim},
};

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

/// Montgomery REDC reduction for odd modulus `m`.
///
/// With `R=βᵏ` and `m·n_prime ≡ −1 (mod β)`, choose each `uᵢ` so limb i of
/// `t + uᵢm` becomes zero. Dividing by β is then a shift. After k steps the
/// value is `t·R⁻¹ (mod m)` and is below `2m`; one conditional subtraction gives
/// the canonical residue. This is invalid for even `m` because `R` has no
/// inverse modulo `m`.
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
