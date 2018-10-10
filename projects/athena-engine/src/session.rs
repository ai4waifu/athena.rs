//! Session and environment.

use crate::polynomial::{PolynomialRequest, PolynomialResult, RingTable, execute_polynomial_with_rings};

/// Mutable evaluation session (bindings, options, ring registry).
#[derive(Debug, Default)]
pub struct Session {
    /// 多项式环 intern 表。
    pub rings: RingTable,
}

impl Session {
    /// 在 session 环表上下文中执行多项式域请求。
    pub fn execute_polynomial(&self, request: PolynomialRequest) -> PolynomialResult {
        execute_polynomial_with_rings(request, &self.rings)
    }
}
