//! `BigFloat` 表示不变量（promotion 见 `numeric_promotion`）。

use athena_numeric::{BigFloat, integer::Sign, natural::Natural};

#[test]
fn normalized_invariant() {
    let bf = BigFloat::try_new(Sign::Positive, Natural::from_u64(8), 0, 8).unwrap();
    assert_eq!(bf.significand().to_u64(), Some(1));
    assert_eq!(bf.exponent(), 3);
}
