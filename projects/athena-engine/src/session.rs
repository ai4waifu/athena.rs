//! Session and environment.

use crate::{
    mgraph::MGraphState,
    polynomial::{
        PolynomialRequest, PolynomialResult, RingTable, execute_polynomial_mgraph, execute_polynomial_with_rings,
    },
};

/// Mutable evaluation session (bindings, options, ring registry, M-Graph).
#[derive(Debug, Default)]
pub struct Session {
    /// 多项式环 intern 表。
    pub rings: RingTable,
    /// M-Graph 状态（多项式缓存 · witness）。
    pub mgraph: MGraphState,
}

impl Session {
    /// 在 session 环表上下文中执行多项式域请求。
    pub fn execute_polynomial(&self, request: PolynomialRequest) -> PolynomialResult {
        execute_polynomial_with_rings(request, &self.rings)
    }

    /// 经 M-Graph 缓存与 witness 记录执行多项式请求。
    pub fn execute_polynomial_mgraph(&mut self, request: PolynomialRequest) -> PolynomialResult {
        execute_polynomial_mgraph(request, &self.rings, &mut self.mgraph)
    }
}
