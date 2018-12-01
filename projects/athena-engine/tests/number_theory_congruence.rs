//! Congruence, CRT and rational reconstruction tests.

use athena_engine::{
    CongruenceSolution, CrtResult, Integer, Modulus, ModulusTable, NumberTheoryRequest, NumberTheoryResult,
    NumberTheoryValue, RationalReconstruction, chinese_remainder, chinese_remainder_pair, execute_number_theory,
    rational_reconstruction, solve_linear_congruence,
};

#[test]
fn linear_unique_class() {
    // 3x ≡ 4 (mod 5) → x ≡ 3 (mod 5) because 3*3=9≡4
    let m = Modulus::new(5).unwrap();
    let out = solve_linear_congruence(&3.into(), &4.into(), &m);
    match out {
        NumberTheoryResult::Exact {
            value: NumberTheoryValue::Congruence(CongruenceSolution::UniqueClass { residue }),
        } => {
            assert_eq!(residue.residue(), &Integer::from_i64(3));
            assert_eq!(residue.modulus(), Some(&m));
        }
        other => panic!("unique: {other:?}"),
    }
}

#[test]
fn linear_multiple_classes() {
    // 2x ≡ 4 (mod 6) → x ≡ 2 (mod 3), multiplicity 2
    let m = Modulus::new(6).unwrap();
    let out = solve_linear_congruence(&2.into(), &4.into(), &m);
    match out {
        NumberTheoryResult::Exact {
            value: NumberTheoryValue::Congruence(CongruenceSolution::MultipleClasses {
                base_residue,
                reduced_modulus,
                ambient_modulus,
                multiplicity,
            }),
        } => {
            assert_eq!(base_residue, Integer::from_i64(2));
            assert_eq!(reduced_modulus.value(), &Integer::from_i64(3));
            assert_eq!(ambient_modulus, m);
            assert_eq!(multiplicity, Integer::from_i64(2));
        }
        other => panic!("multiple: {other:?}"),
    }
}

#[test]
fn linear_no_solution() {
    // 2x ≡ 3 (mod 6) — gcd=2 does not divide 3
    let m = Modulus::new(6).unwrap();
    let out = solve_linear_congruence(&2.into(), &3.into(), &m);
    match out {
        NumberTheoryResult::Exact {
            value: NumberTheoryValue::Congruence(CongruenceSolution::NoSolution { gcd, .. }),
        } => assert_eq!(gcd, Integer::from_i64(2)),
        other => panic!("no solution: {other:?}"),
    }
}

#[test]
fn crt_coprime() {
    // x ≡ 2 (mod 3), x ≡ 3 (mod 5) → x ≡ 8 (mod 15)
    let m3 = Modulus::new(3).unwrap();
    let m5 = Modulus::new(5).unwrap();
    match chinese_remainder_pair(&2.into(), &m3, &3.into(), &m5).unwrap() {
        CrtResult::Consistent { solution, modulus_lcm } => {
            assert_eq!(solution.residue(), &Integer::from_i64(8));
            assert_eq!(modulus_lcm.value(), &Integer::from_i64(15));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn crt_non_coprime_consistent() {
    // x ≡ 2 (mod 4), x ≡ 6 (mod 6) → gcd=2 divides 2-6=-4; lcm=12; x ≡ 6 (mod 12)
    let m4 = Modulus::new(4).unwrap();
    let m6 = Modulus::new(6).unwrap();
    match chinese_remainder_pair(&2.into(), &m4, &6.into(), &m6).unwrap() {
        CrtResult::Consistent { solution, modulus_lcm } => {
            assert_eq!(modulus_lcm.value(), &Integer::from_i64(12));
            assert_eq!(solution.residue().rem(&Integer::from_i64(4)), Integer::from_i64(2));
            assert_eq!(solution.residue().rem(&Integer::from_i64(6)), Integer::from_i64(0)); // 6 mod 6 = 0, wait 6≡0
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn crt_inconsistent() {
    // x ≡ 1 (mod 4), x ≡ 2 (mod 6) — gcd=2 does not divide 1-2
    let m4 = Modulus::new(4).unwrap();
    let m6 = Modulus::new(6).unwrap();
    match chinese_remainder_pair(&1.into(), &m4, &2.into(), &m6).unwrap() {
        CrtResult::Inconsistent { gcd, .. } => assert_eq!(gcd, Integer::from_i64(2)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn crt_multi_via_request() {
    let out = chinese_remainder(
        &[2.into(), 3.into(), 2.into()],
        &[Modulus::new(3).unwrap(), Modulus::new(5).unwrap(), Modulus::new(7).unwrap()],
    );
    match out {
        NumberTheoryResult::Exact {
            value: NumberTheoryValue::Crt(CrtResult::Consistent { solution, modulus_lcm }),
        } => {
            assert_eq!(modulus_lcm.value(), &Integer::from_i64(105));
            assert_eq!(solution.residue(), &Integer::from_i64(23)); // classic
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn rational_recon_half() {
    // 1/2 mod 7 = inv(2) = 4
    let m = Modulus::new(7).unwrap();
    match rational_reconstruction(&4.into(), &m, Some(&1.into()), Some(&2.into())) {
        RationalReconstruction::Found { value } => {
            assert_eq!(value.numerator(), Integer::from_i64(1));
            assert_eq!(value.denominator(), Integer::from_i64(2));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn modulus_table_intern_idempotent() {
    let mut table = ModulusTable::new();
    let m = Modulus::new(17).unwrap();
    let id1 = table.intern(m.clone());
    let id2 = table.intern(m);
    assert_eq!(id1, id2);
    let ctx = table.get(id1).unwrap();
    assert!(ctx.is_odd);
    assert_eq!(ctx.bit_length, Integer::from_i64(17).bits());
}

#[test]
fn domain_linear_congruence() {
    let m = Modulus::new(6).unwrap();
    let out = execute_number_theory(NumberTheoryRequest::SolveLinearCongruence {
        a: 2.into(),
        b: 4.into(),
        modulus: m,
    });
    assert!(matches!(
        out,
        NumberTheoryResult::Exact {
            value: NumberTheoryValue::Congruence(CongruenceSolution::MultipleClasses { .. })
        }
    ));
}
