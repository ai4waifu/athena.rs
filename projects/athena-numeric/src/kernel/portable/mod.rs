//! 可移植（与 ISA 无关）limb 核。
//!
//! 按算法族拆分。limb 与 KernelTable::portable 绑定的默认机器核路径。

#![allow(unused_imports)] // module facade re-exports for `kernel::limb`

mod convenience;
mod div_burnikel_ziegler;
pub(crate) mod div_knuth;
mod div_single;
mod gcd_binary;
mod gcd_half;
mod gcd_lehmer;
mod glue;
mod montgomery;
mod mul_karatsuba;
mod mul_schoolbook;
mod mul_toom3;
mod primitive;
mod scratch_tls;
mod shift;
mod slice_ops;

pub(crate) use crate::algorithm::{MUL_KARATSUBA_THRESHOLD, MUL_TOOM_THRESHOLD, karatsuba_scratch_limbs, toom3_scratch_limbs};
pub(crate) use scratch_tls::with_kernel_scratch;

pub(crate) use convenience::{
    add_n, add_n_budgeted, addmul_1, div_rem, div_rem_budgeted, karatsuba_mul, mul, mul_1, mul_1_budgeted, mul_budgeted, mul_schoolbook, sqr,
    sqr_budgeted, sqr_schoolbook, sub_n, sub_n_budgeted,
};
pub(crate) use gcd_binary::binary_gcd;
pub(crate) use gcd_half::half_gcd;
pub(crate) use gcd_lehmer::gcd;
pub(crate) use glue::{LimbKernel, PortableLimbKernel};
pub(crate) use montgomery::{
    div2_mod, mod_pow_montgomery, mod_pow_montgomery_eligible, mod_pow_montgomery_precomputed, montgomery_precompute,
    mul_mod_montgomery_precomputed,
};
pub(crate) use mul_schoolbook::{
    addmul_1_inplace, addmul_1_inplace_soft, mul_schoolbook_into, mul_schoolbook_into_soft, submul_1_inplace, submul_1_inplace_soft,
};
pub(crate) use primitive::{
    adc, add_1, add_1_2, add_2, cmp_slice, div_rem_1, div_rem_2_1, div_rem_u128, effective_len, is_zero, limbs2_to_u128, mac, mul_1x1, mul_2,
    mul_2x1, mul_wide, normalize_trim, sbb, sub_1, sub_2, sub_2_1,
};
pub(crate) use shift::shr_natural;
pub(crate) use slice_ops::{add_slices_into, sub_slices_into};
