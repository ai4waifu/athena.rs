//! x86_64 machine kernel（ADX/SBB · BMI2 `mulx` schoolbook / `mul_1` / addmul·submul 叶）。
//!
//! - 宽 `mul_into`：Schoolbook 走 `mulx`；Karatsuba/Toom 委托 portable（其叶经
//!   `mul_schoolbook_into` → `mul_schoolbook_mulx` 仍吃 ISA）。
//! - `div_rem_into`：Knuth / Burnikel–Ziegler 在本模块执行（`mulx` addmul·submul 叶 +
//!   ADX 进位链组装 BZ mid/商）。单 limb 仍委派 portable。Portable Knuth 用 soft
//!   addmul/submul 作 parity 基线。
#![allow(unsafe_code)]

use athena_types::Result;

use crate::{
    kernel::{
        buffer::{LimbBuffer, ScratchWorkspace},
        portable::{self, LimbKernel, PortableLimbKernel},
        table::KernelTable,
    },
    policy::execution_budget::ExecutionBudget,
};

mod div;

/// 绑定 x86_64 表（ADX add/sub · BMI2 schoolbook mul/`mul_1` · ISA Knuth/BZ）。
pub fn kernel_table() -> KernelTable {
    KernelTable::from_parts(
        "x86_64_adx",
        add_into_adx,
        sub_into_sbb,
        mul_into_isa,
        mul_1_into_isa,
        sqr_into_isa,
        div::div_rem_into_isa,
        add_1,
        mul_1x1_isa,
    )
}

#[inline]
fn add_1(a: u64, b: u64) -> (u64, u64) {
    #[cfg(target_feature = "adx")]
    {
        unsafe {
            let mut out = 0u64;
            let c = core::arch::x86_64::_addcarry_u64(0, a, b, &mut out);
            (out, u64::from(c))
        }
    }
    #[cfg(not(target_feature = "adx"))]
    {
        let (sum, carry) = a.overflowing_add(b);
        (sum, u64::from(carry))
    }
}

#[inline]
fn mul_1x1_isa(a: u64, b: u64) -> u128 {
    #[cfg(target_feature = "bmi2")]
    {
        unsafe {
            let mut hi = 0u64;
            let lo = core::arch::x86_64::_mulx_u64(a, b, &mut hi);
            (u128::from(hi) << 64) | u128::from(lo)
        }
    }
    #[cfg(not(target_feature = "bmi2"))]
    {
        portable::mul_1x1(a, b)
    }
}

fn add_into_adx(
    a: &[u64],
    b: &[u64],
    out: &mut LimbBuffer,
    _scratch: &mut ScratchWorkspace,
    budget: &ExecutionBudget,
) -> Result<()> {
    let la = portable::effective_len(a);
    let lb = portable::effective_len(b);
    budget.check_add(la, lb)?;
    let n = la.max(lb);
    let storage = out.storage_mut(n + 1, budget)?;
    storage.fill(0);
    let mut carry = 0u8;
    for i in 0..n {
        let ai = *a.get(i).unwrap_or(&0);
        let bi = *b.get(i).unwrap_or(&0);
        let (sum, c) = adc_chain(carry, ai, bi);
        storage[i] = sum;
        carry = c;
    }
    storage[n] = u64::from(carry);
    out.trim_canonical();
    Ok(())
}

fn sub_into_sbb(
    a: &[u64],
    b: &[u64],
    out: &mut LimbBuffer,
    _scratch: &mut ScratchWorkspace,
    budget: &ExecutionBudget,
) -> Result<()> {
    let la = portable::effective_len(a);
    let lb = portable::effective_len(b);
    budget.check_limbs(la.max(lb))?;
    debug_assert!(portable::cmp_slice(a, b) != core::cmp::Ordering::Less);
    let n = la.max(lb);
    let storage = out.storage_mut(n, budget)?;
    storage.fill(0);
    let mut borrow = 0u8;
    for i in 0..n {
        let ai = *a.get(i).unwrap_or(&0);
        let bi = *b.get(i).unwrap_or(&0);
        let (diff, br) = sbb_chain(borrow, ai, bi);
        storage[i] = diff;
        borrow = br;
    }
    debug_assert_eq!(borrow, 0, "natural sub underflow");
    out.trim_canonical();
    Ok(())
}

fn mul_1_into_isa(
    a: &[u64],
    limb: u64,
    out: &mut LimbBuffer,
    _scratch: &mut ScratchWorkspace,
    budget: &ExecutionBudget,
) -> Result<()> {
    let la = portable::effective_len(a);
    budget.check_limbs(la + 1)?;
    if limb == 0 || la == 0 || is_zero_prefix(a, la) {
        return out.set_zero(budget);
    }
    let storage = out.storage_mut(la + 1, budget)?;
    storage.fill(0);
    let mut carry = 0u64;
    for i in 0..la {
        let prod = mul_1x1_isa(a[i], limb) + u128::from(carry);
        storage[i] = prod as u64;
        carry = (prod >> 64) as u64;
    }
    storage[la] = carry;
    out.trim_canonical();
    Ok(())
}

/// Schoolbook `mul_into`：叶乘用 BMI2 `mulx` 环；Karatsuba/Toom 仍委托 portable。
fn mul_into_isa(
    a: &[u64],
    b: &[u64],
    strategy: crate::algorithm::MulStrategy,
    out: &mut LimbBuffer,
    scratch: &mut ScratchWorkspace,
    budget: &ExecutionBudget,
) -> Result<()> {
    use crate::algorithm::MulStrategy;
    match strategy {
        MulStrategy::Zero => out.set_zero(budget),
        MulStrategy::Schoolbook => {
            let la = portable::effective_len(a);
            let lb = portable::effective_len(b);
            budget.check_mul(la, lb)?;
            let need = la + lb;
            let storage = out.storage_mut(need.max(1), budget)?;
            mul_schoolbook_mulx(a, b, storage);
            out.trim_canonical();
            Ok(())
        }
        MulStrategy::Karatsuba | MulStrategy::Toom3 => {
            <PortableLimbKernel as LimbKernel>::mul_into(a, b, strategy, out, scratch, budget)
        }
    }
}

fn sqr_into_isa(
    a: &[u64],
    strategy: crate::algorithm::MulStrategy,
    out: &mut LimbBuffer,
    scratch: &mut ScratchWorkspace,
    budget: &ExecutionBudget,
) -> Result<()> {
    use crate::algorithm::MulStrategy;
    match strategy {
        MulStrategy::Zero => out.set_zero(budget),
        MulStrategy::Schoolbook => {
            // Square via mul(a,a) mulx schoolbook（与 portable sqr 语义一致）。
            mul_into_isa(a, a, MulStrategy::Schoolbook, out, scratch, budget)
        }
        MulStrategy::Karatsuba | MulStrategy::Toom3 => {
            <PortableLimbKernel as LimbKernel>::sqr_into(a, strategy, out, scratch, budget)
        }
    }
}

/// BMI2 `mulx` schoolbook（供 portable Karatsuba/Toom 叶与 `KernelTable` 共用）。
pub(crate) fn mul_schoolbook_mulx(a: &[u64], b: &[u64], out: &mut [u64]) {
    let la = portable::effective_len(a);
    let lb = portable::effective_len(b);
    let need = la + lb;
    debug_assert!(out.len() >= need.max(1));
    out[..need.max(1)].fill(0);
    if la == 0 || lb == 0 || is_zero_prefix(a, la) || is_zero_prefix(b, lb) {
        return;
    }
    for i in 0..la {
        let mut carry = 0u64;
        for j in 0..lb {
            let idx = i + j;
            let prod = mul_1x1_isa(a[i], b[j]) + u128::from(out[idx]) + u128::from(carry);
            out[idx] = prod as u64;
            carry = (prod >> 64) as u64;
        }
        let mut k = i + lb;
        while carry > 0 && k < out.len() {
            let sum = u128::from(out[k]) + u128::from(carry);
            out[k] = sum as u64;
            carry = (sum >> 64) as u64;
            k += 1;
        }
    }
}

/// 就地 `r += a * n`（BMI2 `mulx` + 进位链；无 feature 时软实现）。
pub(crate) fn addmul_1_inplace_isa(r: &mut [u64], a: &[u64], n: u64) -> u64 {
    if n == 0 || is_zero_prefix(a, portable::effective_len(a)) {
        return 0;
    }
    let la = portable::effective_len(a);
    let mut carry = 0u64;
    for i in 0..la {
        let ri = r.get(i).copied().unwrap_or(0);
        let prod = mul_1x1_isa(a[i], n) + u128::from(ri) + u128::from(carry);
        if i < r.len() {
            r[i] = prod as u64;
        }
        carry = (prod >> 64) as u64;
    }
    let mut idx = la;
    while carry > 0 {
        if idx >= r.len() {
            break;
        }
        let sum = u128::from(r[idx]) + u128::from(carry);
        r[idx] = sum as u64;
        carry = (sum >> 64) as u64;
        idx += 1;
    }
    carry
}

/// 就地 `r -= a * n`；下溢返回 `true`。
pub(crate) fn submul_1_inplace_isa(r: &mut [u64], a: &[u64], n: u64) -> bool {
    if n == 0 || is_zero_prefix(a, portable::effective_len(a)) {
        return false;
    }
    let la = portable::effective_len(a);
    let mut borrow = 0u8;
    let mut carry_hi = 0u64;
    for i in 0..r.len() {
        let av = if i < la { a[i] } else { 0 };
        let prod = mul_1x1_isa(av, n) + u128::from(carry_hi);
        let plo = prod as u64;
        carry_hi = (prod >> 64) as u64;
        let (diff, br) = sbb_chain(borrow, r[i], plo);
        r[i] = diff;
        borrow = br;
        if i >= la && carry_hi == 0 && borrow == 0 {
            break;
        }
    }
    borrow != 0 || carry_hi != 0
}

#[inline]
fn is_zero_prefix(a: &[u64], la: usize) -> bool {
    a.iter().take(la).all(|&x| x == 0)
}

#[inline]
pub(super) fn adc_chain(carry_in: u8, a: u64, b: u64) -> (u64, u8) {
    #[cfg(target_feature = "adx")]
    {
        unsafe {
            let mut out = 0u64;
            let c = core::arch::x86_64::_addcarry_u64(carry_in, a, b, &mut out);
            (out, c)
        }
    }
    #[cfg(not(target_feature = "adx"))]
    {
        let (s1, c1) = a.overflowing_add(b);
        let (s2, c2) = s1.overflowing_add(u64::from(carry_in));
        (s2, u8::from(c1 || c2))
    }
}

#[inline]
pub(super) fn sbb_chain(borrow_in: u8, a: u64, b: u64) -> (u64, u8) {
    #[cfg(target_feature = "adx")]
    {
        unsafe {
            let mut out = 0u64;
            let br = core::arch::x86_64::_subborrow_u64(borrow_in, a, b, &mut out);
            (out, br)
        }
    }
    #[cfg(not(target_feature = "adx"))]
    {
        let (d1, b1) = a.overflowing_sub(b);
        let (d2, b2) = d1.overflowing_sub(u64::from(borrow_in));
        (d2, u8::from(b1 || b2))
    }
}
