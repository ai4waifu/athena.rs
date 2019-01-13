//! `BigFloat` / `Decimal` representation invariants (see `exact::promotion` for promotion).

use athena_numeric::{Decimal, integer::Sign, natural::Natural};

#[test]
fn normalized_invariant() {
    let bf = Decimal::try_new(Sign::Positive, Natural::from_u64(8), 0, 8).unwrap();
    assert_eq!(bf.significand().to_u64(), Some(1));
    assert_eq!(bf.exponent(), 3);
}
