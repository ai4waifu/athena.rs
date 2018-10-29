//! 代数域骨架：polynomial / group / field / galois 经 DomainRequest 可分派。

use athena_engine::{
    AthenaEngine, CoefficientDomain, DiagnosticCode, DomainRequest, DomainResult, FieldRequest, GaloisRequest, GroupRequest,
    Integer, MonomialOrder, NumberTheoryRequest, Polynomial, PolynomialRequest, PolynomialResult, RingTable, SymbolId,
};

fn integer_x_ring() -> (RingTable, athena_engine::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    (rings, id)
}

#[test]
fn polynomial_scaffold_unevaluated() {
    let engine = AthenaEngine::new();
    let (_rings, ring) = integer_x_ring();
    let p = Polynomial::zero(ring);
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

    let g =
        engine.execute_domain(DomainRequest::GroupTheory(GroupRequest::Cyclic { order: Integer::from_i64(5) })).expect("ok");
    assert!(matches!(g, DomainResult::GroupTheory(_)));

    let f = engine.execute_domain(DomainRequest::FieldTheory(FieldRequest::Rationals)).expect("ok");
    assert!(matches!(f, DomainResult::FieldTheory(_)));

    let (_rings, ring) = integer_x_ring();
    let gal = engine
        .execute_domain(DomainRequest::GaloisTheory(GaloisRequest::GaloisGroupOfPolynomial {
            polynomial: Polynomial::zero(ring),
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
