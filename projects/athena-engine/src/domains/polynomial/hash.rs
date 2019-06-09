//! 规范多项式稳定 hash（M-Graph / 缓存键；委托 [`super::fingerprint`]）。

use athena_types::Result;

use super::{object::Polynomial, fingerprint::polynomial_fingerprint_u64, ring_table::RingTable};

/// 对 canonical 多项式求稳定结构 hash（不含 Session [`RingId`]）。
pub fn canonical_hash(poly: &Polynomial, rings: &RingTable) -> Result<u64> {
    polynomial_fingerprint_u64(poly, rings)
}
