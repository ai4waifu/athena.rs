//! Directed `f64` rounding primitive tests.

use athena_numeric::rounding::{f64_add_down, f64_add_up, f64_mul_down, f64_mul_up};

#[test]
fn add_directed_brackets_true_sum() {
    let a = 1.0_f64;
    let b = 1.0_f64.next_up();
    let sum = a + b;
    assert!(f64_add_down(a, b) <= sum);
    assert!(f64_add_up(a, b) >= sum);
}

#[test]
fn mul_directed_brackets_product() {
    let a = 1.1_f64;
    let b = 1.1_f64;
    let prod = a * b;
    assert!(f64_mul_down(a, b) <= prod);
    assert!(f64_mul_up(a, b) >= prod);
}
