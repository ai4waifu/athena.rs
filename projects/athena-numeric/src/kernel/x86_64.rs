//! x86_64 machine kernel（ADX/SBB 进位链；乘除复用 pure Rust 以保 parity 基线）。
#![allow(unsafe_code)]

use athena_types::Result;

use crate::{
    kernel::{
        buffer::{LimbBuffer, ScratchWorkspace},
        pure_rust::limb_kernel::{self, LimbKernel, PureRustLimbKernel},
        table::KernelTable,
    },
    policy::execution_budget::ExecutionBudget,
};

/// 绑定 x86_64 表。
pub fn kernel_table() -> KernelTable {
    KernelTable::from_parts(
        "x86_64_adx",
        add_into_adx,
        sub_into_sbb,
        <PureRustLimbKernel as LimbKernel>::mul_into,
        <PureRustLimbKernel as LimbKernel>::mul_1_into,
        <PureRustLimbKernel as LimbKernel>::sqr_into,
        <PureRustLimbKernel as LimbKernel>::div_rem_into,
        add_1,
        limb_kernel::mul_1x1,
    )
}

#[inline]
fn add_1(a: u64, b: u64) -> (u64, u64) {
    #[cfg(target_feature = "adx")]
    {
        // SAFETY: ADX intrinsic。
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

fn add_into_adx(
    a: &[u64],
    b: &[u64],
    out: &mut LimbBuffer,
    _scratch: &mut ScratchWorkspace,
    budget: &ExecutionBudget,
) -> Result<()> {
    let la = limb_kernel::effective_len(a);
    let lb = limb_kernel::effective_len(b);
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
    let la = limb_kernel::effective_len(a);
    let lb = limb_kernel::effective_len(b);
    budget.check_limbs(la.max(lb))?;
    debug_assert!(limb_kernel::cmp_slice(a, b) != core::cmp::Ordering::Less);
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

#[inline]
fn adc_chain(carry_in: u8, a: u64, b: u64) -> (u64, u8) {
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
fn sbb_chain(borrow_in: u8, a: u64, b: u64) -> (u64, u8) {
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
