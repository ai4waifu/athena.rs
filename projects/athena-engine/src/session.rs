//! Session and environment.

use crate::{
    graph_theory::{GraphTheoryRequest, GraphTheoryResult, execute_graph_theory},
    linear_algebra::{LinearAlgebraRequest, LinearAlgebraResult, execute_linear_algebra},
    mgraph::MGraphState,
    polynomial::{PolynomialRequest, PolynomialResult, RingTable, execute_polynomial_mgraph, execute_polynomial_with_rings},
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

    /// 执行图论域请求（E0：与 [`crate::execute_domain`] 的 `GraphTheory` 分支等价）。
    pub fn execute_graph_theory(&self, request: GraphTheoryRequest) -> GraphTheoryResult {
        execute_graph_theory(request)
    }

    /// 执行线性代数域请求（与 [`crate::execute_domain`] 的 `LinearAlgebra` 分支等价）。
    pub fn execute_linear_algebra(&self, request: LinearAlgebraRequest) -> LinearAlgebraResult {
        execute_linear_algebra(request)
    }
}
