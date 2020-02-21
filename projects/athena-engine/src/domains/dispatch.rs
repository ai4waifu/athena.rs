//! 顶层域分派 — `DomainRequest` / `DomainResult`。
//!
//! Living `28`：`DomainRequest` → [`plan_domain`] → [`DomainPlan`] → [`interpret_domain_plan`] → `DomainResult`。
//! 微积分、数论、多项式、群、域、伽罗瓦、图论、线性代数、优化经此入口进入 `athena-engine`。

use athena_types::Diagnostic;

use crate::{
    domains::{
        calculus::{CalculusRequest, CalculusResult, CalculusValue, execute_calculus},
        field::{FieldRequest, FieldResult, execute_field_with_table_mut},
        galois::{GaloisRequest, GaloisResult, execute_galois_with_tables},
        graph_theory::{GraphTheoryRequest, GraphTheoryResult, execute_graph_theory},
        group::{GroupRequest, GroupResult, execute_group_with_table_mut},
        linear_algebra::{LinearAlgebraRequest, LinearAlgebraResult, execute_linear_algebra},
        number_theory::{NumberTheoryRequest, NumberTheoryResult, execute_number_theory},
        optimization::{OptimizationRequest, OptimizationResult, execute_optimization},
        plan_exec::interpret_domain_plan,
        planner::plan_domain,
        polynomial::{PolynomialRequest, PolynomialResult, execute_polynomial_with_rings},
        views::SeriesPolynomialView,
    },
    runtime::session::Session,
};

/// 顶层域请求。
#[derive(Debug, PartialEq)]
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
    /// 优化与规划（Living `16`，非 Solve 别名）。
    Optimization(OptimizationRequest),
}

/// 顶层域结果 — 按域区分，禁止压成单一类型 map。
#[derive(Debug, PartialEq)]
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
    /// 优化结果。
    Optimization(OptimizationResult),
}

/// 分派顶层 [`DomainRequest`]（经 DomainPlanner 产出 [`DomainPlan`] 并逐步解释）。
///
/// 微积分分支读写 `session` arena；其余域暂不依赖 session。
pub fn execute_domain(session: &mut Session, request: DomainRequest) -> Result<DomainResult, Diagnostic> {
    let plan = plan_domain(&request);
    let (result, _report) = interpret_domain_plan(session, &plan, request, call_domain_provider)?;
    Ok(result)
}

/// Invoke the owning domain provider (`DomainPlan` `CallDomainProvider` body).
pub(crate) fn call_domain_provider(session: &mut Session, request: DomainRequest) -> Result<DomainResult, Diagnostic> {
    match request {
        DomainRequest::Calculus(req) => Ok(DomainResult::Calculus(execute_calculus(session, req))),
        DomainRequest::NumberTheory(req) => Ok(DomainResult::NumberTheory(execute_number_theory(req))),
        DomainRequest::Polynomial(req) => {
            Ok(DomainResult::Polynomial(execute_polynomial_with_rings(req, &session.rings, &session.polynomial_objects)))
        }
        DomainRequest::GroupTheory(req) => Ok(DomainResult::GroupTheory(execute_group_with_table_mut(req, &mut session.groups))),
        DomainRequest::FieldTheory(req) => Ok(DomainResult::FieldTheory(execute_field_with_table_mut(req, session.rings.field_table_mut()))),
        DomainRequest::GaloisTheory(req) => {
            Ok(DomainResult::GaloisTheory(execute_galois_with_tables(req, session.rings.field_table_mut(), &mut session.groups)))
        },
        DomainRequest::GraphTheory(req) => Ok(DomainResult::GraphTheory(execute_graph_theory(req))),
        DomainRequest::LinearAlgebra(req) => Ok(DomainResult::LinearAlgebra(execute_linear_algebra(req, &session.matrix_objects))),
        DomainRequest::Optimization(req) => Ok(DomainResult::Optimization(execute_optimization(req))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::context::DomainExecutionContext;
    use athena_ir::SemanticOperator;
    use athena_types::AssumptionSet;

    #[test]
    fn series_plan_opens_series_polynomial_view() {
        let mut session = Session::new();
        let (expression, variable, center) = {
            let dc = DomainExecutionContext::new(&mut session);
            let variable = dc.intern("x");
            let xs = dc.symbol_id(variable);
            let center = dc.in_(0);
            let expression = dc.apply_semantic(SemanticOperator::Unary(athena_ir::UnaryFunction::Sin), vec![xs]);
            (expression, variable, center)
        };
        let result = execute_domain(
            &mut session,
            DomainRequest::Calculus(CalculusRequest::Series { expression, variable, center, order: 2, assumptions: AssumptionSet::empty() }),
        )
        .expect("execute");
        match result {
            DomainResult::Calculus(
                CalculusResult::Exact { value: CalculusValue::Series(r), .. }
                | CalculusResult::Conditional { value: CalculusValue::Series(r), .. }
                | CalculusResult::Unevaluated { expression: CalculusValue::Series(r), .. },
            ) => {
                assert!(SeriesPolynomialView::open(&session.series_objects, r).is_some());
            }
            other => panic!("expected series DomainResult, got {other:?}"),
        }
    }
}
