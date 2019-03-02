//! Portable (ISA-agnostic) limb kernel.
//!
//! Split by algorithm family. Default machine-kernel path for `limb` and
//! `KernelTable::portable` binding.

#![allow(unused_imports)] // module facade re-exports for `kernel::limb`

mod scratch_tls;
mod primitive;
mod slice_ops;
mod mul_schoolbook;
mod mul_karatsuba;
mod mul_toom3;
mod div_single;
mod div_knuth;
mod div_burnikel_ziegler;
mod shift;
mod glue;
mod convenience;
mod gcd_binary;
mod gcd_lehmer;
mod gcd_half;
mod montgomery;

pub(crate) use scratch_tls::with_kernel_scratch;
pub(crate) use crate::algorithm::{
    MUL_KARATSUBA_THRESHOLD, MUL_TOOM_THRESHOLD, karatsuba_scratch_limbs, toom3_scratch_limbs,
};

pub(crate) use primitive::{
    adc, add_1, add_1_2, add_2, cmp_slice, div_rem_1, div_rem_2_1, div_rem_u128, effective_len,
    is_zero, limbs2_to_u128, mac, mul_1x1, mul_2, mul_2x1, mul_wide, normalize_trim, sbb, sub_1,
    sub_2, sub_2_1,
};
pub(crate) use mul_schoolbook::{addmul_1_inplace, mul_schoolbook_into, submul_1_inplace};
pub(crate) use slice_ops::{add_slices_into, sub_slices_into};
pub(crate) use convenience::{
    add_n, add_n_budgeted, addmul_1, div_rem, div_rem_budgeted, karatsuba_mul, mul, mul_1,
    mul_1_budgeted, mul_budgeted, mul_schoolbook, sqr, sqr_budgeted, sqr_schoolbook, sub_n,
    sub_n_budgeted,
};
pub(crate) use gcd_lehmer::gcd;
pub(crate) use gcd_binary::binary_gcd;
pub(crate) use gcd_half::half_gcd;
pub(crate) use montgomery::{
    div2_mod, mod_pow_montgomery, mod_pow_montgomery_eligible, mod_pow_montgomery_precomputed,
    montgomery_precompute, mul_mod_montgomery_precomputed,
};
pub(crate) use shift::shr_natural;
pub(crate) use glue::{LimbKernel, PortableLimbKernel};
