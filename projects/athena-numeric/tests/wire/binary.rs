//! 二进制 wire 往返与 ANV1 Int/Rat/Real/Complex/Interval/Modular reject 矩阵。

use athena_numeric::{
    BranchPolicy, Complex, Integer, Interval, IntervalDecoration, ModularValue, Modulus, NumericValue, NumericValueWire,
    Rational, Real,
};
use athena_types::NumericKind;
use std::str::FromStr;

fn mag_bytes(count: u32, limbs: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + limbs.len() * 8);
    out.extend_from_slice(&count.to_le_bytes());
    for &limb in limbs {
        out.extend_from_slice(&limb.to_le_bytes());
    }
    out
}

fn int_wire(sign: u8, payload: Vec<u8>) -> NumericValueWire {
    NumericValueWire {
        kind: NumericKind::Integer,
        domain_payload: Vec::new(),
        payload,
        sign,
        precision: athena_numeric::PrecisionInfo::exact(),
        version: NumericValueWire::current_version(),
    }
}

fn rat_wire(sign: u8, payload: Vec<u8>) -> NumericValueWire {
    NumericValueWire {
        kind: NumericKind::Rational,
        domain_payload: Vec::new(),
        payload,
        sign,
        precision: athena_numeric::PrecisionInfo::exact(),
        version: NumericValueWire::current_version(),
    }
}

fn reason_of(err: &athena_types::Diagnostic) -> Option<&str> {
    match err.details.get("reason") {
        Some(athena_types::DiagnosticValue::Text(s)) => Some(s.as_str()),
        _ => None,
    }
}

#[test]
fn binary_integer_roundtrip_large() {
    let n = Integer::from_str("999999999999999999999999999999").unwrap();
    let v = NumericValue::integer(n);
    let wire = NumericValueWire::encode(&v).unwrap();
    assert!(!wire.payload.is_empty());
    assert!(!std::str::from_utf8(&wire.payload).is_ok_and(|s| s.chars().all(|c| c.is_ascii_digit())));
    let blob = wire.to_bytes().unwrap();
    assert_eq!(&blob[0..4], b"ANV1");
    let back = NumericValueWire::from_bytes(&blob).unwrap().decode().unwrap();
    assert_eq!(back, v);
}

#[test]
fn binary_rational_roundtrip() {
    let r = Rational::new(Integer::from_i64(-3), Integer::from_i64(8));
    let v = NumericValue::rational(r);
    let wire = NumericValueWire::encode(&v).unwrap();
    let back = wire.decode().unwrap();
    assert_eq!(back, v);
}

#[test]
fn text_format_explicit_path() {
    let text = NumericValueWire::encode_text(&NumericValue::small_int(42)).unwrap();
    assert_eq!(text, "42");
    let wire = NumericValueWire::decode_text(NumericKind::Integer, "42").unwrap();
    assert_eq!(wire.decode().unwrap(), NumericValue::small_int(42));
}

#[test]
fn blob_stable_for_same_value() {
    let v = NumericValue::integer(Integer::from_i64(12345));
    let w1 = NumericValueWire::encode(&v).unwrap().to_bytes().unwrap();
    let w2 = NumericValueWire::encode(&v).unwrap().to_bytes().unwrap();
    assert_eq!(w1, w2);
}

#[test]
fn reject_mag_count_zero() {
    let err = int_wire(0, mag_bytes(0, &[])).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("mag_count_zero"));
}

#[test]
fn reject_mag_trailing_zero() {
    let err = int_wire(1, mag_bytes(2, &[1, 0])).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("mag_trailing_zero"));
}

#[test]
fn reject_sign_zero_nonzero_mag() {
    let err = int_wire(0, mag_bytes(1, &[42])).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("sign_zero_nonzero_mag"));
}

#[test]
fn reject_sign_pos_zero_mag() {
    let err = int_wire(1, mag_bytes(1, &[0])).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("sign_pos_zero_mag"));
}

#[test]
fn reject_sign_neg_zero_mag() {
    let err = int_wire(2, mag_bytes(1, &[0])).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("sign_neg_zero_mag"));
}

#[test]
fn reject_sign_unknown() {
    let err = int_wire(3, mag_bytes(1, &[1])).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("sign_unknown"));
}

#[test]
fn reject_rational_denom_zero() {
    let mut payload = mag_bytes(1, &[1]);
    payload.extend(mag_bytes(1, &[0]));
    let err = rat_wire(1, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("rational_denom_zero"));
}

#[test]
fn reject_rational_unreduced() {
    let mut payload = mag_bytes(1, &[2]);
    payload.extend(mag_bytes(1, &[4]));
    let err = rat_wire(1, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("rational_unreduced"));
}

#[test]
fn reject_rational_zero_denom_not_one() {
    let mut payload = mag_bytes(1, &[0]);
    payload.extend(mag_bytes(1, &[2]));
    let err = rat_wire(0, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("rational_zero_denom_not_one"));
}

#[test]
fn reject_rational_trailing() {
    let mut payload = mag_bytes(1, &[1]);
    payload.extend(mag_bytes(1, &[1]));
    payload.push(0xAB);
    let err = rat_wire(1, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("rational_trailing"));
}

#[test]
fn accept_canonical_zero_integer() {
    let wire = int_wire(0, mag_bytes(1, &[0]));
    assert_eq!(wire.decode().unwrap(), NumericValue::integer(Integer::zero()));
}

#[test]
fn accept_canonical_half_rational() {
    let mut payload = mag_bytes(1, &[1]);
    payload.extend(mag_bytes(1, &[2]));
    let wire = rat_wire(1, payload);
    let back = wire.decode().unwrap();
    assert_eq!(back, NumericValue::rational(Rational::new(Integer::from_i64(1), Integer::from_i64(2))));
}

fn real_wire(sign: u8, payload: Vec<u8>) -> NumericValueWire {
    NumericValueWire {
        kind: NumericKind::Real,
        domain_payload: Vec::new(),
        payload,
        sign,
        precision: athena_numeric::PrecisionInfo::machine(),
        version: NumericValueWire::current_version(),
    }
}

fn real_decimal_wire(sign: u8, payload: Vec<u8>, precision_bits: u32) -> NumericValueWire {
    NumericValueWire {
        kind: NumericKind::Real,
        domain_payload: Vec::new(),
        payload,
        sign,
        precision: athena_numeric::PrecisionInfo::arbitrary(precision_bits),
        version: NumericValueWire::current_version(),
    }
}

#[test]
fn binary_real_machine_roundtrip() {
    for x in [0.0, -0.0, 1.5, -2.25, f64::INFINITY, f64::NEG_INFINITY, f64::MIN_POSITIVE] {
        let v = NumericValue::machine(x);
        let wire = NumericValueWire::encode(&v).unwrap();
        let back = wire.decode().unwrap();
        match (v, back) {
            (NumericValue::Real(athena_numeric::Real::Machine(a)), NumericValue::Real(athena_numeric::Real::Machine(b))) => {
                assert_eq!(a.to_bits(), b.to_bits(), "bits for {x}");
            }
            _ => panic!("expected machine real"),
        }
    }
}

#[test]
fn binary_real_decimal_roundtrip() {
    let d = athena_numeric::Decimal::from_f64(1.25).unwrap();
    let v = NumericValue::decimal(d);
    let wire = NumericValueWire::encode(&v).unwrap();
    let back = wire.decode().unwrap();
    assert_eq!(back, v);
}

#[test]
fn reject_real_machine_nan() {
    let mut payload = vec![0u8];
    payload.extend_from_slice(&f64::NAN.to_bits().to_le_bytes());
    let err = real_wire(0, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("real_machine_nan"));
}

#[test]
fn reject_encode_real_machine_nan() {
    let err = NumericValueWire::encode(&NumericValue::machine(f64::NAN)).unwrap_err();
    assert_eq!(reason_of(&err), Some("real_machine_nan"));
}

#[test]
fn reject_real_machine_len() {
    let err = real_wire(0, vec![0, 1, 2, 3]).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("real_machine_len"));
}

#[test]
fn reject_real_unknown_subtype() {
    let err = real_wire(0, vec![9]).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("real_unknown_subtype"));
}

#[test]
fn reject_real_decimal_precision_zero() {
    let mut payload = vec![1u8];
    payload.extend(mag_bytes(1, &[1]));
    payload.extend_from_slice(&0i64.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    let err = real_decimal_wire(1, payload, 0).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("real_decimal_precision_zero"));
}

#[test]
fn reject_real_decimal_not_normalized() {
    let mut payload = vec![1u8];
    payload.extend(mag_bytes(1, &[12])); // even significand
    payload.extend_from_slice(&0i64.to_le_bytes());
    payload.extend_from_slice(&53u32.to_le_bytes());
    let err = real_decimal_wire(1, payload, 53).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("real_decimal_not_normalized"));
}

#[test]
fn reject_real_decimal_precision_exceeds() {
    let mut payload = vec![1u8];
    payload.extend(mag_bytes(1, &[0xFFFF_FFFF_FFFF_FFFF])); // 64 bits
    payload.extend_from_slice(&0i64.to_le_bytes());
    payload.extend_from_slice(&8u32.to_le_bytes());
    let err = real_decimal_wire(1, payload, 8).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("real_decimal_precision_exceeds"));
}

#[test]
fn reject_real_decimal_trailing() {
    let mut payload = vec![1u8];
    payload.extend(mag_bytes(1, &[1]));
    payload.extend_from_slice(&0i64.to_le_bytes());
    payload.extend_from_slice(&53u32.to_le_bytes());
    payload.push(0xAB);
    let err = real_decimal_wire(1, payload, 53).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("real_decimal_trailing"));
}

/// 轻量 fuzz：随机改写 Real Machine blob 字节，合法则 round-trip，非法则带 reason。
#[test]
fn fuzz_real_machine_blob_mutations() {
    let base = NumericValueWire::encode(&NumericValue::machine(std::f64::consts::PI)).unwrap().to_bytes().unwrap();
    for i in 0..base.len() {
        for delta in [1u8, 0x7f, 0xff] {
            let mut mut_bytes = base.clone();
            mut_bytes[i] = mut_bytes[i].wrapping_add(delta);
            match NumericValueWire::from_bytes(&mut_bytes).and_then(|w| w.decode()) {
                Ok(v) => {
                    let again = NumericValueWire::encode(&v).unwrap().to_bytes().unwrap();
                    let back = NumericValueWire::from_bytes(&again).unwrap().decode().unwrap();
                    assert_eq!(back, v);
                }
                Err(e) => {
                    assert!(e.details.get("reason").is_some() || e.details.get("operation").is_some());
                }
            }
        }
    }
}

fn interval_wire(payload: Vec<u8>) -> NumericValueWire {
    NumericValueWire {
        kind: NumericKind::Interval,
        domain_payload: Vec::new(),
        payload,
        sign: 0,
        precision: athena_numeric::PrecisionInfo::exact(),
        version: NumericValueWire::current_version(),
    }
}

fn complex_wire(sign: u8, payload: Vec<u8>) -> NumericValueWire {
    NumericValueWire {
        kind: NumericKind::Complex,
        domain_payload: Vec::new(),
        payload,
        sign,
        precision: athena_numeric::PrecisionInfo::exact(),
        version: NumericValueWire::current_version(),
    }
}

fn nested_machine_real(x: f64) -> Vec<u8> {
    let mut out = vec![0u8, 0]; // Machine nested：header sign 恒 0 + subtype machine
    out.extend_from_slice(&x.to_bits().to_le_bytes());
    out
}

#[test]
fn binary_complex_machine_roundtrip() {
    let z = Complex {
        re: Real::machine(1.25),
        im: Real::machine(-2.5),
        branch: BranchPolicy::Principal,
    };
    let v = NumericValue::complex(z);
    let back = NumericValueWire::encode(&v).unwrap().decode().unwrap();
    assert_eq!(back, v);

    let z2 = Complex {
        re: Real::machine(0.0),
        im: Real::machine(1.0),
        branch: BranchPolicy::RealOnly,
    };
    let v2 = NumericValue::complex(z2);
    let back2 = NumericValueWire::encode(&v2).unwrap().decode().unwrap();
    assert_eq!(back2, v2);
}

#[test]
fn reject_complex_unknown_branch() {
    let mut payload = vec![9u8];
    payload.extend(nested_machine_real(1.0));
    payload.extend(nested_machine_real(2.0));
    let err = complex_wire(0, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("complex_unknown_branch"));
}

#[test]
fn reject_complex_trailing() {
    let mut payload = vec![0u8];
    payload.extend(nested_machine_real(1.0));
    payload.extend(nested_machine_real(2.0));
    payload.push(0xAB);
    let err = complex_wire(0, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("complex_trailing"));
}

#[test]
fn reject_complex_sign_nonzero() {
    let mut payload = vec![0u8];
    payload.extend(nested_machine_real(1.0));
    payload.extend(nested_machine_real(0.0));
    let err = complex_wire(1, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("sign_unknown"));
}

#[test]
fn reject_complex_truncated_nested() {
    let payload = vec![0u8, 0, 0]; // branch + incomplete nested real
    let err = complex_wire(0, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("complex_trailing"));
}

fn modular_wire(sign: u8, payload: Vec<u8>) -> NumericValueWire {
    NumericValueWire {
        kind: NumericKind::Modular,
        domain_payload: Vec::new(),
        payload,
        sign,
        precision: athena_numeric::PrecisionInfo::exact(),
        version: NumericValueWire::current_version(),
    }
}

#[test]
fn binary_interval_roundtrip_variants() {
    let empty = NumericValue::interval(Interval::empty());
    let entire = NumericValue::interval(Interval::entire_with(IntervalDecoration::Defined));
    let bounded = NumericValue::interval(
        Interval::try_bounded(Real::machine(-1.0), Real::machine(2.5), IntervalDecoration::Certain).unwrap(),
    );
    for v in [empty, entire, bounded] {
        let back = NumericValueWire::encode(&v).unwrap().decode().unwrap();
        assert_eq!(back, v);
    }
}

#[test]
fn binary_modular_roundtrip() {
    let m = Modulus::new(Integer::from_i64(7)).unwrap();
    let v = NumericValue::modular(ModularValue::new(Integer::from_i64(10), m));
    let back = NumericValueWire::encode(&v).unwrap().decode().unwrap();
    assert_eq!(back, v);
}

#[test]
fn reject_interval_unknown_subtype() {
    let err = interval_wire(vec![9]).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("interval_unknown_subtype"));
}

#[test]
fn reject_interval_bad_bounds() {
    // Bounded + Certain + nested Machine lower=2.0 + upper=1.0
    let mut payload = vec![2u8, 0];
    // nested real: sign + subtype machine + bits
    payload.push(0);
    payload.push(0);
    payload.extend_from_slice(&2.0f64.to_bits().to_le_bytes());
    payload.push(0);
    payload.push(0);
    payload.extend_from_slice(&1.0f64.to_bits().to_le_bytes());
    let err = interval_wire(payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("interval_bad_bounds"));
}

#[test]
fn reject_modular_bad_modulus() {
    let mut payload = mag_bytes(1, &[1]);
    payload.extend(mag_bytes(1, &[1])); // modulus = 1
    let err = modular_wire(1, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("modular_bad_modulus"));
}

#[test]
fn reject_modular_residue_unreduced() {
    let mut payload = mag_bytes(1, &[9]);
    payload.extend(mag_bytes(1, &[7]));
    let err = modular_wire(1, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("modular_residue_unreduced"));
}

#[test]
fn reject_modular_trailing() {
    let mut payload = mag_bytes(1, &[3]);
    payload.extend(mag_bytes(1, &[7]));
    payload.push(0xAB);
    let err = modular_wire(1, payload).decode().unwrap_err();
    assert_eq!(reason_of(&err), Some("modular_trailing"));
}
