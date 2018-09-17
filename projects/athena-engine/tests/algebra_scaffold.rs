//! 代数域骨架：polynomial / group / field / galois 经 DomainRequest 可分派。

use num_bigint::BigInt;

use athena_engine::{
    AthenaEngine, CoefficientRing, DiagnosticCode, DomainRequest, DomainResult, FieldRequest, GaloisRequest, GroupRequest,
    NumberTheoryRequest, Polynomial, PolynomialRequest, PolynomialResult,
};

#[test]
fn polynomial_scaffold_unevaluated() {
    let engine = AthenaEngine::new();
    let p = Polynomial::zero(CoefficientRing::Integer, vec!["x".into()]);
    let out = engine.execute_domain(DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: p })).expect("ok");
    match out {
        DomainResult::Polynomial(PolynomialResult::Unevaluated { reason }) => {
            assert_eq!(reason.code, DiagnosticCode::UnsupportedOperation);
            assert_eq!(reason.details.get("domain").map(|v| v.to_string()).as_deref(), Some("polynomial"));
        }
        other => panic!("expected polynomial Unevaluated, got {other:?}"),
    }
}

#[test]
fn group_field_galois_scaffolds_unevaluated() {
    let engine = AthenaEngine::new();

    let g = engine.execute_domain(DomainRequest::GroupTheory(GroupRequest::Cyclic { order: BigInt::from(5) })).expect("ok");
    assert!(matches!(g, DomainResult::GroupTheory(_)));

    let f = engine.execute_domain(DomainRequest::FieldTheory(FieldRequest::Rationals)).expect("ok");
    assert!(matches!(f, DomainResult::FieldTheory(_)));

    let gal = engine
        .execute_domain(DomainRequest::GaloisTheory(GaloisRequest::GaloisGroup {
            polynomial: Polynomial::zero(CoefficientRing::Rational, vec!["x".into()]),
            base_field: athena_engine::FieldId(0),
        }))
        .expect("ok");
    assert!(matches!(gal, DomainResult::GaloisTheory(_)));
}

#[test]
fn number_theory_congruence_scaffold() {
    let engine = AthenaEngine::new();
    let out = engine
        .execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::SolveLinearCongruence {
            a: 2.into(),
            b: 4.into(),
            modulus: 6.into(),
        }))
        .expect("ok");
    match out {
        DomainResult::NumberTheory(athena_engine::NumberTheoryResult::Unevaluated { reason }) => {
            assert_eq!(reason.code, DiagnosticCode::UnsupportedOperation);
            assert_eq!(reason.details.get("operation").map(|v| v.to_string()).as_deref(), Some("solve_linear_congruence"));
        }
        other => panic!("expected congruence scaffold, got {other:?}"),
    }
}
