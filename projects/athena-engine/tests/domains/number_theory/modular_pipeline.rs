//! Montgomery/Barrett 内核、`ModulusId` 路径、批量逆元、分解流水线。

use athena_engine::domains::number_theory::{
    FactorAlgorithms, FactorLimits, FactorizationCompleteness, NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue, batch_mod_inverse,
    execute_number_theory, factor_integer, mod_inverse_with_table, mod_pow_with_table, verify_factorization,
};
use athena_numeric::{Integer, Modulus, ModulusTable};

#[test]
fn modulus_context_precomputes_montgomery_and_barrett() {
    use athena_numeric::ModulusContext;
    use std::str::FromStr;
    let narrow = Modulus::new(65537).unwrap();
    let narrow_ctx = ModulusContext::from_modulus(narrow);
    assert!(narrow_ctx.barrett.is_some());
    let wide = Modulus::new(Integer::from_str("18446744073709551657").unwrap()).unwrap();
    let wide_ctx = ModulusContext::from_modulus(wide);
    assert!(wide_ctx.montgomery.is_some());
    assert!(wide_ctx.barrett.is_some());
}

#[test]
fn mod_pow_with_table_matches_direct() {
    let m = Modulus::new(97).unwrap();
    let mut table = ModulusTable::new();
    let mv = m.value();
    let direct = Integer::from_i64(3).mod_pow(&Integer::from_i64(10), &mv).expect("mod_pow");
    let via = mod_pow_with_table(&3.into(), &10.into(), &m, &mut table).unwrap();
    assert_eq!(via.residue(), direct);
    assert!(via.modulus_id().is_some());
}

#[test]
fn batch_mod_inverse_product_tree() {
    let p = Modulus::new(101).unwrap();
    let residues = vec![2.into(), 3.into(), 5.into()];
    let mut table = ModulusTable::new();
    let invs = batch_mod_inverse(&residues, &p, &mut table).unwrap();
    assert_eq!(invs.len(), 3);
    for (r, inv) in residues.iter().zip(invs.iter()) {
        let inv_r = inv.residue();
        let pv = p.value();
        let prod = r.mul(&inv_r).rem(&pv).expect("rem");
        assert_eq!(prod, Integer::one());
    }
}

#[test]
fn batch_mod_inverse_via_request() {
    let out =
        execute_number_theory(NumberTheoryRequest::BatchModInverse { residues: vec![2.into(), 3.into()], modulus: Modulus::new(11).unwrap() });
    match out {
        NumberTheoryResult::Exact { value: NumberTheoryValue::ModularList(v) } => {
            assert_eq!(v.len(), 2);
            assert_eq!(v[0].residue().mul(&2.into()).rem(&Integer::from_i64(11)).expect("rem"), Integer::one());
            assert_eq!(v[1].residue().mul(&3.into()).rem(&Integer::from_i64(11)).expect("rem"), Integer::one());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn pollard_rho_splits_semiprime() {
    let n = Integer::from_i64(10403).mul(&Integer::from_i64(104729));
    let mut limits = FactorLimits::default();
    limits.policy.algorithms = FactorAlgorithms::with_pipeline();
    limits.budget.max_trial = 100;
    limits.budget.max_steps = Some(500_000);
    let f = factor_integer(&n, &limits).expect("factor");
    assert_eq!(f.completeness(), FactorizationCompleteness::Complete);
    verify_factorization(&n, &f).expect("verify semiprime");
    assert!(f.factors.len() >= 2);
}

#[test]
fn fermat_splits_close_semiprime() {
    let n = Integer::from_i64(1_000_003).mul(&Integer::from_i64(1_000_033));
    let mut limits = FactorLimits::default();
    limits.policy.algorithms = FactorAlgorithms { trial: false, pollard_rho: false, pollard_p1: false, ecm: false, quadratic_sieve: true };
    limits.budget.max_steps = Some(100_000);
    let f = factor_integer(&n, &limits).expect("factor");
    assert_eq!(f.completeness(), FactorizationCompleteness::Complete);
}

#[test]
fn mod_inverse_interned() {
    let m = Modulus::new(17).unwrap();
    let mut table = ModulusTable::new();
    let v = mod_inverse_with_table(&3.into(), &m, &mut table).unwrap();
    assert!(v.modulus_id().is_some());
    let mv = m.value();
    assert_eq!(v.residue().mul(&3.into()).rem(&mv).expect("rem"), Integer::one());
}
