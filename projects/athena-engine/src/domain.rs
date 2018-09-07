//! 顶层域分派 — `DomainRequest` / `DomainResult`。
//!
//! 微积分与数论等域经此入口进入 `athena-engine`；M-Graph 闭包落地后亦由此编排。

use athena_types::Diagnostic;

use crate::{
    calculus::{CalculusRequest, CalculusResult, CalculusValue, execute_calculus},
    number_theory::{NumberTheoryRequest, NumberTheoryResult, execute_number_theory},
};

/// 顶层域请求。
#[derive(Debug, Clone, PartialEq)]
pub enum DomainRequest {
    /// 微积分 / 高等数学。
    Calculus(CalculusRequest),
    /// 数论。
    NumberTheory(NumberTheoryRequest),
}

/// 顶层域结果 — 按域区分，禁止压成无类型 map。
#[derive(Debug, Clone, PartialEq)]
pub enum DomainResult {
    /// 微积分条件结果。
    Calculus(CalculusResult<CalculusValue>),
    /// 数论结果。
    NumberTheory(NumberTheoryResult),
}

/// 分派顶层 [`DomainRequest`]。
pub fn execute_domain(request: DomainRequest) -> Result<DomainResult, Diagnostic> {
    match request {
        DomainRequest::Calculus(req) => Ok(DomainResult::Calculus(execute_calculus(req))),
        DomainRequest::NumberTheory(req) => Ok(DomainResult::NumberTheory(execute_number_theory(req))),
    }
}
