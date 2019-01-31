//! 二进制 wire 往返与 ANV1 Int/Rat reject 矩阵。

use athena_numeric::{Integer, NumericValue, NumericValueWire, Rational};
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
    assert_eq!(
        back,
        NumericValue::rational(Rational::new(Integer::from_i64(1), Integer::from_i64(2)))
    );
}
