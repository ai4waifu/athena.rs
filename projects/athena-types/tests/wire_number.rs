//! Wire number parse and normalization tests.

use athena_types::wire::{ExactNumber, WireNumber};

#[test]
fn rational_i64_normalizes() {
    let e = ExactNumber::rational_i64(2, 4);
    assert_eq!(e, ExactNumber::Rational { numer: "1".into(), denom: "2".into() });
}

#[test]
fn from_exact_literal_rational() {
    let n = WireNumber::from_exact_literal("3/4").unwrap();
    assert_eq!(n.to_render_string(), "3/4");
}

#[test]
fn from_decimal_str_big() {
    let n = WireNumber::from_decimal_str("99999999999999999999").unwrap();
    assert_eq!(n.to_render_string(), "99999999999999999999");
}
