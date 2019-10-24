//! Living `29` 顶层语义入口：Goal → Obligation → Reflector → Plan / Result。
//!
//! `execute_domain` 只应出现在本模块的 `NeedComputation` 分支内，不得冒充顶层语义路径。
//! 查询面绑定 [`Session::mgraph`] 的 semantic core，禁止每次宿主调用新建空 `MGraphCore`。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    api::request::DomainGoal,
    domains::{
        calculus::{CalculusRequest, CalculusResult, CalculusValue},
        dispatch::{DomainRequest, DomainResult, execute_domain},
        planner::{PlanStep, plan_domain},
    },
    reasoning::mgraph::{
        AdmissionGate, CalculusReflector, CalculusRelationKind, MGraphView, ObjectRef, PolynomialReflector, ProofObligation,
        Reflection, RelationRef, ScopeRef, SemanticReflector, TheoryContextId, VerificationPolicy, predicates,
    },
    runtime::session::Session,
};

/// 语义入口对一次 [`DomainGoal`] 的结果（Living `29`）。
#[derive(Debug, PartialEq)]
pub enum DomainSemanticOutcome {
    /// M-Graph 已有足够强的 admitted relation。
    AlreadyKnown {
        /// 已接纳关系。
        relation: RelationRef,
    },
    /// Reflector 选出 DomainPlan 后经 provider 算出的结果。
    Computed(DomainResult),
    /// 缺少领域对象（须构造 DomainObject / lowering）。
    NeedObject {
        /// 机器标识（非前端名）。
        object_kind: &'static str,
    },
    /// 缺少显式域映射。
    NeedConversion {
        /// 源域标签。
        source: &'static str,
        /// 目标域标签。
        target: &'static str,
    },
    /// 资源 / 搜索未完成。
    Inconclusive,
}

/// 从 [`DomainRequest`] 构造缺口义务（无则返回 `None`，走通用 NeedComputation）。
///
/// 多项式请求会先 intern 进 [`Session::polynomial_objects`]，义务携带 `PolynomialRef` 对应的 [`ObjectRef`]。
pub fn obligation_from_domain_request(session: &mut Session, request: &DomainRequest) -> Option<ProofObligation> {
    match request {
        DomainRequest::Calculus(CalculusRequest::Derivative { expression, variable, .. }) => Some(ProofObligation {
            predicate: predicates::DERIVATIVE_OF,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(expression.0)),
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(variable.0)),
            ],
        }),
        DomainRequest::Calculus(CalculusRequest::Integral { expression, variable, .. })
        | DomainRequest::Calculus(CalculusRequest::DefiniteIntegral { expression, variable, .. }) => Some(ProofObligation {
            predicate: predicates::INTEGRAL_OF,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(expression.0)),
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(variable.0)),
            ],
        }),
        DomainRequest::Calculus(CalculusRequest::Series { expression, variable, .. })
        | DomainRequest::Calculus(CalculusRequest::Laurent { expression, variable, .. })
        | DomainRequest::Calculus(CalculusRequest::Asymptotic { expression, variable, .. }) => Some(ProofObligation {
            predicate: predicates::SERIES_EXPANSION,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(expression.0)),
                ObjectRef::new(TheoryContextId::CALCULUS, u64::from(variable.0)),
            ],
        }),
        DomainRequest::Polynomial(poly_req) => {
            let interned = crate::domains::polynomial::intern_request_object_refs(
                poly_req,
                &session.rings,
                &mut session.polynomial_objects,
            )
            .unwrap_or_default();
            let known_objects = match crate::domains::polynomial::cache_key_for_request(poly_req, &session.rings) {
                Ok(key) => vec![ObjectRef::new(TheoryContextId::POLYNOMIAL, key.fingerprint())],
                Err(_) => interned,
            };
            Some(ProofObligation {
                predicate: predicates::POLYNOMIAL_RESULT,
                scope: ScopeRef::UNCONDITIONAL,
                known_objects,
            })
        }
        _ => None,
    }
}

fn reflect_domain(request: &DomainRequest, obligation: &ProofObligation, view: &MGraphView<'_>) -> Reflection {
    match request {
        DomainRequest::Calculus(_) => CalculusReflector.reflect(obligation, view),
        DomainRequest::Polynomial(_) => PolynomialReflector.reflect(obligation, view),
        _ => Reflection::NeedComputation {
            plan: plan_domain(request),
        },
    }
}

fn calculus_kind_from_predicate(predicate: crate::reasoning::mgraph::PredicateId) -> Option<CalculusRelationKind> {
    if predicate == predicates::DERIVATIVE_OF {
        Some(CalculusRelationKind::DerivativeOf)
    }
    else if predicate == predicates::INTEGRAL_OF {
        Some(CalculusRelationKind::IntegralOf)
    }
    else if predicate == predicates::SERIES_EXPANSION {
        Some(CalculusRelationKind::SeriesExpansion)
    }
    else {
        None
    }
}

fn try_admit_calculus_exact(session: &mut Session, obligation: &ProofObligation, result: &DomainResult) {
    let DomainResult::Calculus(CalculusResult::Exact {
        value: CalculusValue::Expression(result_term),
        conditions,
    }) = result
    else {
        return;
    };
    if !conditions.is_empty() {
        return;
    }
    let Some(kind) = calculus_kind_from_predicate(obligation.predicate) else {
        return;
    };
    if obligation.known_objects.len() < 2 {
        return;
    }
    let expression_fingerprint = obligation.known_objects[0].fingerprint;
    let variable_fingerprint = obligation.known_objects[1].fingerprint;
    let _ = AdmissionGate::admit_calculus_relation(
        &mut session.mgraph.semantic,
        kind,
        expression_fingerprint,
        variable_fingerprint,
        *result_term,
        &VerificationPolicy::default(),
    );
}

fn materialize_already_known(session: &Session, relation: RelationRef) -> Result<DomainResult, Diagnostic> {
    let Some(record) = session.mgraph.semantic.relation(relation) else {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "semantic_entry")
            .detail("reason", "relation_missing")
            .arg("relation", relation.0));
    };
    match &record.verified.claim.proposition {
        crate::reasoning::mgraph::Proposition::CalculusRelation { result_term, .. } => {
            Ok(DomainResult::Calculus(CalculusResult::Exact {
                value: CalculusValue::Expression(*result_term),
                conditions: Vec::new(),
            }))
        }
        crate::reasoning::mgraph::Proposition::PolynomialResult { request_fingerprint, .. } => {
            let Some(entry) = session
                .mgraph
                .operational
                .result_cache
                .polynomial
                .get_by_request_fingerprint(*request_fingerprint)
            else {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "semantic_entry")
                    .detail("reason", "polynomial_cache_miss")
                    .arg("relation", relation.0));
            };
            Ok(DomainResult::Polynomial(entry.result.clone()))
        }
        _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "semantic_entry")
            .detail("reason", "already_known_not_materialized")
            .arg("relation", relation.0)),
    }
}

/// Living `29` 语义入口：先查 [`Session::mgraph`]，再允许 `NeedComputation` → Living 28 Plan → provider。
pub fn execute_domain_goal(session: &mut Session, goal: DomainGoal) -> Result<DomainSemanticOutcome, Diagnostic> {
    let DomainGoal::Dispatch(request) = goal;
    let obligation = obligation_from_domain_request(session, &request);
    let reflection = {
        let view = session.mgraph.semantic.view();
        match &obligation {
            Some(obligation) => reflect_domain(&request, obligation, &view),
            None => Reflection::NeedComputation {
                plan: plan_domain(&request),
            },
        }
    };
    match reflection {
        Reflection::AlreadyKnown { relation } => Ok(DomainSemanticOutcome::AlreadyKnown { relation }),
        Reflection::NeedObject { object_kind } => Ok(DomainSemanticOutcome::NeedObject { object_kind }),
        Reflection::NeedConversion { source, target } => Ok(DomainSemanticOutcome::NeedConversion { source, target }),
        Reflection::NeedRelation { .. } => Ok(DomainSemanticOutcome::Inconclusive),
        Reflection::Inconclusive => Ok(DomainSemanticOutcome::Inconclusive),
        Reflection::NeedComputation { plan } => {
            if !plan.steps.iter().any(|s| matches!(s, PlanStep::CallDomainProvider)) {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "semantic_entry")
                    .detail("reason", "plan missing CallDomainProvider"));
            }
            let result = match request {
                DomainRequest::Polynomial(req) => {
                    let poly = crate::domains::polynomial::execute_polynomial_mgraph(req, &session.rings, &mut session.mgraph);
                    DomainResult::Polynomial(poly)
                }
                other => {
                    let result = execute_domain(session, other)?;
                    if let Some(obligation) = &obligation {
                        try_admit_calculus_exact(session, obligation, &result);
                    }
                    result
                }
            };
            Ok(DomainSemanticOutcome::Computed(result))
        }
    }
}

/// Project a semantic outcome into [`DomainResult`] for host APIs that still expect provider payloads.
pub fn domain_result_from_semantic_outcome(session: &Session, outcome: DomainSemanticOutcome) -> Result<DomainResult, Diagnostic> {
    match outcome {
        DomainSemanticOutcome::Computed(result) => Ok(result),
        DomainSemanticOutcome::AlreadyKnown { relation } => materialize_already_known(session, relation),
        DomainSemanticOutcome::NeedObject { object_kind } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "semantic_entry")
            .detail("reason", "need_object")
            .detail("object_kind", object_kind)),
        DomainSemanticOutcome::NeedConversion { source, target } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "semantic_entry")
            .detail("reason", "need_conversion")
            .detail("source", source)
            .detail("target", target)),
        DomainSemanticOutcome::Inconclusive => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "semantic_entry")
            .detail("reason", "inconclusive")),
    }
}

/// Host convenience：[`Session::mgraph`] + [`execute_domain_goal`] + project to [`DomainResult`]。
///
/// Living `29`：公共宿主应走此路径，而不是直接调用 [`execute_domain`]。
pub fn execute_domain_via_semantic_entry(session: &mut Session, request: DomainRequest) -> Result<DomainResult, Diagnostic> {
    let outcome = execute_domain_goal(session, DomainGoal::Dispatch(request))?;
    domain_result_from_semantic_outcome(session, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::{
        calculus::{CalculusRequest, CalculusResult, CalculusValue, DerivativeOrder},
        context::DomainExecutionContext,
    };
    use athena_ir::SemanticOperator;
    use athena_types::{AssumptionSet, SymbolId, TermId};

    #[test]
    fn calculus_goal_computes_when_session_graph_empty() {
        let mut session = Session::new();
        let goal = DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
            variable: SymbolId(0),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        }));
        match execute_domain_goal(&mut session, goal).expect("ok") {
            DomainSemanticOutcome::Computed(DomainResult::Calculus(_)) => {}
            other => panic!("expected Computed calculus, got {other:?}"),
        }
    }

    #[test]
    fn calculus_second_goal_is_already_known_after_exact_admit() {
        let mut session = Session::new();
        let (expression, variable) = {
            let dc = DomainExecutionContext::new(&mut session);
            let variable = dc.intern("x");
            let xs = dc.symbol_id(variable);
            let three = dc.in_(3);
            let expression = dc.apply_semantic(SemanticOperator::Power, vec![xs, three]);
            (expression, variable)
        };
        let make_goal = || {
            DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Derivative {
                expression,
                variable,
                order: DerivativeOrder::First,
                assumptions: AssumptionSet::empty(),
            }))
        };
        let first = execute_domain_goal(&mut session, make_goal()).expect("first");
        let DomainSemanticOutcome::Computed(DomainResult::Calculus(CalculusResult::Exact {
            value: CalculusValue::Expression(term),
            ..
        })) = first
        else {
            panic!("expected Exact Expression first, got {first:?}");
        };
        assert!(session.mgraph.semantic.relation_count() >= 1);
        let second = execute_domain_goal(&mut session, make_goal()).expect("second");
        match second {
            DomainSemanticOutcome::AlreadyKnown { relation } => {
                let replayed = domain_result_from_semantic_outcome(&session, DomainSemanticOutcome::AlreadyKnown { relation })
                    .expect("materialize");
                assert_eq!(
                    replayed,
                    DomainResult::Calculus(CalculusResult::Exact {
                        value: CalculusValue::Expression(term),
                        conditions: Vec::new(),
                    })
                );
            }
            other => panic!("expected AlreadyKnown, got {other:?}"),
        }
    }

    #[test]
    fn polynomial_goal_computes_when_request_carries_polynomial() {
        use crate::domains::polynomial::{Polynomial, PolynomialRequest};
        let mut session = Session::new();
        let goal = DomainGoal::Dispatch(DomainRequest::Polynomial(PolynomialRequest::Normalize {
            polynomial: Polynomial::zero(athena_types::RingId(0)),
        }));
        match execute_domain_goal(&mut session, goal).expect("ok") {
            DomainSemanticOutcome::Computed(DomainResult::Polynomial(_)) => {}
            other => panic!("expected Computed polynomial, got {other:?}"),
        }
    }

    #[test]
    fn polynomial_second_goal_is_already_known_after_mgraph_admit() {
        use crate::domains::polynomial::{
            CoefficientDomain, MonomialOrder, Polynomial, PolynomialBuilder, PolynomialRequest, PolynomialResult,
        };
        use athena_types::SymbolId;
        let mut session = Session::new();
        let ring = session
            .rings
            .intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex)
            .expect("ring");
        let polynomial = PolynomialBuilder::new(ring).build(&session.rings).expect("zero poly");
        let make_goal = || {
            DomainGoal::Dispatch(DomainRequest::Polynomial(PolynomialRequest::Normalize {
                polynomial: polynomial.owning_copy(),
            }))
        };
        let first = execute_domain_goal(&mut session, make_goal()).expect("first");
        let DomainSemanticOutcome::Computed(DomainResult::Polynomial(PolynomialResult::Exact { value })) = first else {
            panic!("expected Exact polynomial first, got {first:?}");
        };
        assert!(session.mgraph.semantic.relation_count() >= 1);
        let second = execute_domain_goal(&mut session, make_goal()).expect("second");
        match second {
            DomainSemanticOutcome::AlreadyKnown { relation } => {
                let replayed = domain_result_from_semantic_outcome(&session, DomainSemanticOutcome::AlreadyKnown { relation })
                    .expect("materialize");
                assert_eq!(replayed, DomainResult::Polynomial(PolynomialResult::Exact { value }));
            }
            other => panic!("expected AlreadyKnown, got {other:?}"),
        }
    }
}
