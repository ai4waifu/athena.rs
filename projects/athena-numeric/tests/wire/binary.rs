//! Binary wire roundtrip and text/binary separation tests.

use athena_numeric::{Integer, NumericValue, NumericValueWire};
use athena_types::NumericKind;

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
    use athena_numeric::Rational;
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

use std::str::FromStr;
