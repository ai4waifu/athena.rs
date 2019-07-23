//! ANV1 多 kind 轻量 LCG fuzz：合法则 round-trip，非法须带 `reason` / `operation`。

use athena_numeric::{
    AlgebraicNumber, AlgebraicRepresentation, BranchPolicy, Complex, Decimal, FiniteFieldValue, Integer, Interval, IntervalDecoration,
    ModularValue, Modulus, NumericValue, NumericValueWire, PAdicValue, PolynomialFingerprint, Rational, Real,
};
use athena_types::FieldId;

fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state
}

fn assert_mutation_contract(bytes: &[u8]) {
    match NumericValueWire::from_bytes(bytes).and_then(|w| w.decode()) {
        Ok(v) => {
            let again = NumericValueWire::encode(&v).unwrap().to_bytes().unwrap();
            let back = NumericValueWire::from_bytes(&again).unwrap().decode().unwrap();
            assert_eq!(back, v);
        }
        Err(e) => {
            assert!(
                e.details.get("reason").is_some() || e.details.get("operation").is_some(),
                "ANV1 reject must carry reason or operation: {e:?}"
            );
        }
    }
}

fn fuzz_base_blob(base: &[u8], seed: u64, rounds: usize) {
    let mut state = seed;
    for _ in 0..rounds {
        let mut mut_bytes = base.to_vec();
        let n_flips = (lcg_next(&mut state) as usize % 4) + 1;
        for _ in 0..n_flips {
            if mut_bytes.is_empty() {
                break;
            }
            let i = (lcg_next(&mut state) as usize) % mut_bytes.len();
            let delta = (lcg_next(&mut state) as u8).wrapping_add(1);
            mut_bytes[i] = mut_bytes[i].wrapping_add(delta);
        }
        // 偶发截断 / 追加，覆盖 trailing 与 header 长度合同
        match lcg_next(&mut state) % 5 {
            0 if mut_bytes.len() > 8 => {
                let cut = (lcg_next(&mut state) as usize) % mut_bytes.len();
                mut_bytes.truncate(cut.max(4));
            }
            1 => mut_bytes.push(lcg_next(&mut state) as u8),
            2 => {
                let insert_at = (lcg_next(&mut state) as usize) % (mut_bytes.len() + 1);
                mut_bytes.insert(insert_at, lcg_next(&mut state) as u8);
            }
            _ => {}
        }
        assert_mutation_contract(&mut_bytes);
    }
}

fn encode_blob(v: &NumericValue) -> Vec<u8> {
    NumericValueWire::encode(v).unwrap().to_bytes().unwrap()
}

#[test]
fn fuzz_anv1_multi_kind_lcg_mutations() {
    let bases: Vec<(u64, Vec<u8>)> = vec![
        (0xA001, encode_blob(&NumericValue::integer(Integer::from_i64(-12345)))),
        (0xA002, encode_blob(&NumericValue::rational(Rational::new(Integer::from_i64(-3), Integer::from_i64(8))))),
        (0xA003, encode_blob(&NumericValue::machine(std::f64::consts::E))),
        (0xA004, encode_blob(&NumericValue::decimal(Decimal::from_f64(1.25).unwrap()))),
        (
            0xA005,
            encode_blob(&NumericValue::complex(Complex { re: Real::machine(1.25), im: Real::machine(-2.5), branch: BranchPolicy::Principal })),
        ),
        (
            0xA006,
            encode_blob(&NumericValue::interval(
                Interval::try_bounded(Real::machine(-1.0), Real::machine(2.5), IntervalDecoration::Certain).unwrap(),
            )),
        ),
        (0xA007, encode_blob(&NumericValue::modular(ModularValue::new(Integer::from_i64(10), Modulus::new(Integer::from_i64(7)).unwrap())))),
        (
            0xA008,
            encode_blob(&NumericValue::algebraic(
                AlgebraicNumber::try_new(
                    PolynomialFingerprint(7),
                    Interval::try_bounded(Real::machine(1.4), Real::machine(1.5), IntervalDecoration::Certain).unwrap(),
                    AlgebraicRepresentation::MinimalPolynomial { polynomial: PolynomialFingerprint(7), root_index: 0 },
                )
                .unwrap(),
            )),
        ),
        (
            0xA009,
            encode_blob(&NumericValue::finite_field(
                FiniteFieldValue::try_new(FieldId(4), athena_types::FieldPresentationId(2), vec![Integer::from_i64(1), Integer::from_i64(-2)])
                    .unwrap(),
            )),
        ),
        (0xA00A, encode_blob(&NumericValue::padic(PAdicValue::from_integer(&Integer::from_i64(12), Integer::from_i64(5), 4).unwrap()))),
    ];

    for (seed, base) in bases {
        assert!(!base.is_empty());
        assert_eq!(&base[0..4], b"ANV1");
        fuzz_base_blob(&base, seed, 256);
    }
}
