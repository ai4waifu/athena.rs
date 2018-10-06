//! Session and environment.

use crate::polynomial::RingTable;

/// Mutable evaluation session (bindings, options, ring registry).
#[derive(Debug, Default)]
pub struct Session {
    /// 多项式环 intern 表。
    pub rings: RingTable,
}
