//! # 用途
//! Montgomery 约化（REDC）、Montgomery 乘法与模幂，
//! 适用于奇数模。
//!
//! # 数学模型
//! 设 `R = βᵏ > m` 且 `m' ≡ −m⁻¹ (mod β)`，REDC 仅用移位与乘加
//! 将 `t` 映到 `t R⁻¹ mod m`。在 Montgomery 形式
//! `ã = a R mod m` 下，模乘变为乘积的 REDC。
//!
//! # 推导
//! 取每个 `uᵢ = tᵢ m' mod β`，使 `t + uᵢ m` 的第 `i` 个 limb 清零。
//! `k` 步后条件减法得到 `< m` 的剩余。
//!
//! # 算法步骤
//! 1. `montgomery_nprime` / `montgomery_precompute`（`R² mod m`）。
//! 2. 乘以 `R²` 再 REDC 以转入；用 REDC 相乘；再转出。
//! 3. `mod_pow_montgomery_*`：在 Montgomery 域内平方-乘。
//!
//! # 前置条件
//! - 奇数模；宽度 `≥ MONTGOMERY_THRESHOLD`。
//! - 偶数模不得走此路径（`mod_pow_montgomery_eligible`）。
//!
//! # 后置条件
//! - 结果为 `[0,m)` 中的规范剩余。
//!
//! # 复杂度
//! 幂运算 `O(log e)` 次 Montgomery 乘；每次 REDC 为 `O(k²)` limb 工作。
//!
//! # 交叉阈值
//! 模为奇数且足够宽时使用；小/偶模走通用路径。
//!
//! # 失败模式
//! 偶 `m` 时 `R` 无逆；须通过资格门控。
//!
//! # 测试
//! `tests/exact/` 下模运算 / `mod_pow` 套件及差分纯测试。

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

/// 奇数模 `m` 的 Montgomery REDC 约化。
///
/// 设 `R=βᵏ` 且 `m·n_prime ≡ −1 (mod β)`，选每个 `uᵢ` 使
/// `t + uᵢm` 的第 i 个 limb 为零。除以 β 即移位。`k` 步后值为
/// `t·R⁻¹ (mod m)` 且小于 `2m`；一次条件减法给出规范剩余。
/// 对偶 `m` 无效，因 `R` 在模 `m` 下无逆。
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

pub(crate) fn mod_pow_montgomery_precomputed(base: &[u64], exp: &[u64], modulus: &[u64], n_prime: u64, r2_mod_m: &[u64]) -> Vec<u64> {
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
