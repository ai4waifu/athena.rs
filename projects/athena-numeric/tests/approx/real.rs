//! Real 表示冒烟测试。

use athena_numeric::{Decimal, Real};

#[test]
fn machine_finite_and_non_finite() {
    assert!(Real::machine(1.0).is_finite());
    assert!(!Real::machine(f64::NAN).is_finite());
    assert!(Real::decimal(Decimal::zero()).is_finite());
}
