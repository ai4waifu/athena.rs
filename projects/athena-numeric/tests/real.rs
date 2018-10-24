//! Real representation smoke tests.

use athena_numeric::{BigFloat, Real};

#[test]
fn machine_finite_and_non_finite() {
    assert!(Real::machine(1.0).is_finite());
    assert!(!Real::machine(f64::NAN).is_finite());
    assert!(Real::big_float(BigFloat::zero()).is_finite());
}
