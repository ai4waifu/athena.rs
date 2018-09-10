//! 数论域 gate：gcd / 素性 / 分解 / 模运算。

use num_bigint::BigInt;

use athena_engine::{
    AthenaEngine, DiagnosticCode, DomainRequest, DomainResult, FactorLimits, FactorizationCompleteness, Modulus,
    NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue, Primality,
};

fn expect_nt(r: Result<DomainResult, athena_types::Diagnostic>) -> NumberTheoryResult {
    match r.expect("domain ok") {
        DomainResult::NumberTheory(v) => v,
        other => panic!("expected NumberTheory domain, got {other:?}"),
    }
}

#[test]
fn gcd_lcm_egcd_via_domain() {
    let engine = AthenaEngine::new();
    let out =
        expect_nt(engine.execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd { a: 48.into(), b: 18.into() })));
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::Integer(g) } => assert_eq!(g, BigInt::from(6)),
        other => panic!("gcd: {other:?}"),
    }

    let out =
        expect_nt(engine.execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::Lcm { a: 4.into(), b: 6.into() })));
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::Integer(l) } => assert_eq!(l, BigInt::from(12)),
        other => panic!("lcm: {other:?}"),
    }

    let out = expect_nt(
        engine.execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::ExtendedGcd { a: 240.into(), b: 46.into() })),
    );
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::ExtendedGcd(e) } => {
            assert_eq!(e.g, BigInt::from(2));
            assert_eq!(&e.s * BigInt::from(240) + &e.t * BigInt::from(46), e.g);
        }
        other => panic!("egcd: {other:?}"),
    }
}

#[test]
fn primality_distinguishes_probable() {
    let engine = AthenaEngine::new();
    let out = expect_nt(engine.execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::PrimalityTest {
        n: 97.into(),
        miller_rabin_rounds: None,
    })));
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::Primality(Primality::Prime) } => {}
        other => panic!("97 should be Prime: {other:?}"),
    }

    let out = expect_nt(engine.execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::PrimalityTest {
        n: 91.into(),
        miller_rabin_rounds: None,
    })));
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::Primality(Primality::Composite) } => {}
        other => panic!("91 composite: {other:?}"),
    }
}

#[test]
fn factor_integer_complete_small() {
    let engine = AthenaEngine::new();
    let out = expect_nt(engine.execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::FactorInteger {
        n: (-360).into(),
        limits: FactorLimits::default(),
    })));
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::Factorization(f) } => {
            assert_eq!(f.unit, BigInt::from(-1));
            assert_eq!(f.completeness, FactorizationCompleteness::Complete);
            assert_eq!(f.remainder, BigInt::from(1));
            // 360 = 2^3 * 3^2 * 5
            assert_eq!(f.factors.len(), 3);
            assert_eq!(f.factors[0].base, BigInt::from(2));
            assert_eq!(f.factors[0].exponent, 3);
        }
        other => panic!("factor: {other:?}"),
    }
}

#[test]
fn modular_inverse_and_pow() {
    let engine = AthenaEngine::new();
    let m = Modulus::new(17).unwrap();
    let out = expect_nt(
        engine.execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::ModInverse { a: 3.into(), modulus: m.clone() })),
    );
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::Modular(v) } => {
            assert_eq!(v.residue(), &BigInt::from(6)); // 3*6=18≡1
            assert_eq!(v.modulus(), &m);
        }
        other => panic!("inv: {other:?}"),
    }

    let out = expect_nt(engine.execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::ModPow {
        base: 3.into(),
        exp: 5.into(),
        modulus: m,
    })));
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::Modular(v) } => assert_eq!(v.residue(), &BigInt::from(5)), /* 243 ≡ 5 (mod 17) */
        other => panic!("pow: {other:?}"),
    }
}

#[test]
fn modular_inverse_missing() {
    let engine = AthenaEngine::new();
    let m = Modulus::new(15).unwrap();
    let out = expect_nt(
        engine.execute_domain(DomainRequest::NumberTheory(NumberTheoryRequest::ModInverse { a: 6.into(), modulus: m })),
    );
    match out {
        NumberTheoryResult::Unevaluated { reason } => {
            assert_eq!(reason.code, DiagnosticCode::ModularInverseMissing);
        }
        other => panic!("expected Unevaluated, got {other:?}"),
    }
}

#[test]
fn modulus_invalid() {
    let err = Modulus::new(1).unwrap_err();
    assert_eq!(err.code, DiagnosticCode::ModulusInvalid);
}
