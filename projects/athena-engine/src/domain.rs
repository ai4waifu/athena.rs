//! 顶层域分派 — `DomainRequest` / `DomainResult`。
//!
//! 微积分、数论、多项式、群、域、伽罗瓦、图论经此入口进入 `athena-engine`。

use athena_types::Diagnostic;

use crate::{
    calculus::{CalculusRequest, CalculusResult, CalculusValue, execute_calculus},
    field::{FieldRequest, FieldResult, execute_field},
    galois::{GaloisRequest, GaloisResult, execute_galois},
    graph_theory::{GraphTheoryRequest, GraphTheoryResult, execute_graph_theory},
    group::{GroupRequest, GroupResult, execute_group},
    linear_algebra::{LinearAlgebraRequest, LinearAlgebraResult, execute_linear_algebra},
    number_theory::{NumberTheoryRequest, NumberTheoryResult, execute_number_theory},
    polynomial::{PolynomialRequest, PolynomialResult, execute_polynomial},
};

/// 顶层域请求。
#[derive(Debug, Clone, PartialEq)]
pub enum DomainRequest {
    /// 微积分 / 高等数学。
    Calculus(CalculusRequest),
    /// 数论。
    NumberTheory(NumberTheoryRequest),
    /// 多项式代数。
    Polynomial(PolynomialRequest),
    /// 群论。
    GroupTheory(GroupRequest),
    /// 域论。
    FieldTheory(FieldRequest),
    /// 伽罗瓦理论。
    GaloisTheory(GaloisRequest),
    /// 图论。
    GraphTheory(GraphTheoryRequest),
    /// 线性代数。
    LinearAlgebra(LinearAlgebraRequest),
}

/// 顶层域结果 — 按域区分，禁止压成无类型 map。
#[derive(Debug, Clone, PartialEq)]
pub enum DomainResult {
    /// 微积分条件结果。
    Calculus(CalculusResult<CalculusValue>),
    /// 数论结果。
    NumberTheory(NumberTheoryResult),
    /// 多项式结果。
    Polynomial(PolynomialResult),
    /// 群论结果。
    GroupTheory(GroupResult),
    /// 域论结果。
    FieldTheory(FieldResult),
    /// 伽罗瓦结果。
    GaloisTheory(GaloisResult),
    /// 图论结果。
    GraphTheory(GraphTheoryResult),
    /// 线性代数结果。
    LinearAlgebra(LinearAlgebraResult),
}

/// 分派顶层 [`DomainRequest`]。
pub fn execute_domain(request: DomainRequest) -> Result<DomainResult, Diagnostic> {
    match request {
        DomainRequest::Calculus(req) => Ok(DomainResult::Calculus(execute_calculus(req))),
        DomainRequest::NumberTheory(req) => Ok(DomainResult::NumberTheory(execute_number_theory(req))),
        DomainRequest::Polynomial(req) => Ok(DomainResult::Polynomial(execute_polynomial(req))),
        DomainRequest::GroupTheory(req) => Ok(DomainResult::GroupTheory(execute_group(req))),
        DomainRequest::FieldTheory(req) => Ok(DomainResult::FieldTheory(execute_field(req))),
        DomainRequest::GaloisTheory(req) => Ok(DomainResult::GaloisTheory(execute_galois(req))),
        DomainRequest::GraphTheory(req) => Ok(DomainResult::GraphTheory(execute_graph_theory(req))),
        DomainRequest::LinearAlgebra(req) => Ok(DomainResult::LinearAlgebra(execute_linear_algebra(req))),
    }
}
