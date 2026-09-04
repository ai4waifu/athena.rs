//! P2 算术工具链测试。

use std::str::FromStr;

use athena_engine::domains::number_theory::{
    NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue, PrimeIterator, execute_number_theory, is_perfect_power, isqrt, jacobi_symbol,
    kronecker_symbol, next_prime_after, perfect_power_decomposition, primes_up_to,
};
use athena_numeric::{Integer, Modulus, ModulusContext};

#[test]
fn isqrt_floor() {
    assert_eq!(isqrt(&Integer::from_i64(10)), Integer::from_i64(3));
    assert_eq!(isqrt(&Integer::from_i64(16)), Integer::from_i64(4));
    assert_eq!(isqrt(&Integer::from_i64(-5)), Integer::zero());
}

#[test]
fn perfect_power_detects_eight() {
    let (b, e) = perfect_power_decomposition(&Integer::from_i64(8)).unwrap();
    assert_eq!(b, Integer::from_i64(2));
    assert_eq!(e, 3);
    assert!(is_perfect_power(&Integer::from_i64(8)));
    assert!(!is_perfect_power(&Integer::from_i64(12)));
}

#[test]
fn jacobi_and_kronecker() {
    assert_eq!(jacobi_symbol(&2.into(), &3.into()), Some(-1));
    assert_eq!(jacobi_symbol(&2.into(), &15.into()), Some(1));
    assert_eq!(kronecker_symbol(&2.into(), &15.into()), 1);
    assert_eq!(kronecker_symbol(&Integer::from_i64(-2), &Integer::from_i64(-1)), -1);
}

#[test]
fn prime_iterator_and_sieve() {
    let ps: Vec<_> = PrimeIterator::from_start(10).take(3).collect();
    assert_eq!(ps, vec![11.into(), 13.into(), 17.into()]);
    assert_eq!(next_prime_after(&10.into()), Integer::from_i64(11));
    assert_eq!(primes_up_to(10), vec![2, 3, 5, 7].into_iter().map(Integer::from_i64).collect::<Vec<_>>());
}

#[test]
fn domain_isqrt_request() {
    let out = execute_number_theory(NumberTheoryRequest::Isqrt { n: 24.into() });
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::Integer(v) } => assert_eq!(v, Integer::from_i64(4)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn domain_primes_up_to() {
    let out = execute_number_theory(NumberTheoryRequest::PrimesUpTo { limit: 7 });
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::IntegerList(v) } => assert_eq!(v.len(), 4),
        other => panic!("{other:?}"),
    }
}

#[test]
fn wide_modulus_montgomery_precompute() {
    let wide = Modulus::new(Integer::from_str("18446744073709551657").unwrap()).unwrap();
    let ctx = ModulusContext::from_modulus(wide);
    assert!(ctx.montgomery.is_some());
}
