//! Directed IEEE binary64 rounding for interval enclosures.
//!
//! Outward rounding uses error-free reversal checks (Boost.Interval style), not
//! re-rounded reverse operations.

use std::cmp::Ordering;

/// `a + b` rounded toward −∞.
pub fn f64_add_down(a: f64, b: f64) -> f64 {
    let mut r = a + b;
    if !r.is_finite() {
        return r;
    }
    if (r - a) > b || (r - b) > a {
        r = r.next_down();
    }
    r
}

/// `a + b` rounded toward +∞.
pub fn f64_add_up(a: f64, b: f64) -> f64 {
    let mut r = a + b;
    if !r.is_finite() {
        return r;
    }
    if (r - a) < b || (r - b) < a {
        r = r.next_up();
    }
    r
}

/// `a - b` rounded toward −∞.
pub fn f64_sub_down(a: f64, b: f64) -> f64 {
    let mut r = a - b;
    if !r.is_finite() {
        return r;
    }
    if (a - r) > b || (r + b) > a {
        r = r.next_down();
    }
    r
}

/// `a - b` rounded toward +∞.
pub fn f64_sub_up(a: f64, b: f64) -> f64 {
    let mut r = a - b;
    if !r.is_finite() {
        return r;
    }
    if (a - r) < b || (r + b) < a {
        r = r.next_up();
    }
    r
}

/// `a * b` rounded toward −∞.
pub fn f64_mul_down(a: f64, b: f64) -> f64 {
    let mut r = a * b;
    if !r.is_finite() || r == 0.0 {
        return r;
    }
    if mul_rounded_too_high_for_down(a, b, r) {
        r = r.next_down();
    }
    r
}

/// `a * b` rounded toward +∞.
pub fn f64_mul_up(a: f64, b: f64) -> f64 {
    let mut r = a * b;
    if !r.is_finite() || r == 0.0 {
        return r;
    }
    if mul_rounded_too_low_for_up(a, b, r) {
        r = r.next_up();
    }
    r
}

/// `a / b` rounded toward −∞.
pub fn f64_div_down(a: f64, b: f64) -> f64 {
    let mut r = a / b;
    if !r.is_finite() {
        return r;
    }
    if div_rounded_too_high_for_down(a, b, r) {
        r = r.next_down();
    }
    r
}

/// `a / b` rounded toward +∞.
pub fn f64_div_up(a: f64, b: f64) -> f64 {
    let mut r = a / b;
    if !r.is_finite() {
        return r;
    }
    if div_rounded_too_low_for_up(a, b, r) {
        r = r.next_up();
    }
    r
}

fn sign_class(x: f64) -> Ordering {
    if x.is_nan() || x == 0.0 {
        Ordering::Equal
    }
    else if x.is_sign_positive() {
        Ordering::Greater
    }
    else {
        Ordering::Less
    }
}

fn mul_rounded_too_high_for_down(a: f64, b: f64, r: f64) -> bool {
    match (sign_class(a), sign_class(b)) {
        (Ordering::Greater, Ordering::Greater) => r / a > b || r / b > a,
        (Ordering::Greater, Ordering::Less) => r / a < b || r / b > a,
        (Ordering::Less, Ordering::Greater) => r / a < b || r / b > a,
        (Ordering::Less, Ordering::Less) => r / a > b || r / b < a,
        _ => false,
    }
}

fn mul_rounded_too_low_for_up(a: f64, b: f64, r: f64) -> bool {
    match (sign_class(a), sign_class(b)) {
        (Ordering::Greater, Ordering::Greater) => r / a < b || r / b < a,
        (Ordering::Greater, Ordering::Less) => r / a > b || r / b < a,
        (Ordering::Less, Ordering::Greater) => r / a > b || r / b < a,
        (Ordering::Less, Ordering::Less) => r / a < b || r / b > a,
        _ => false,
    }
}

fn div_rounded_too_high_for_down(a: f64, b: f64, r: f64) -> bool {
    match (sign_class(a), sign_class(b)) {
        (Ordering::Greater, Ordering::Greater) => r * b > a || a / r > b,
        (Ordering::Greater, Ordering::Less) => r * b < a || a / r < b,
        (Ordering::Less, Ordering::Greater) => r * b < a || a / r > b,
        (Ordering::Less, Ordering::Less) => r * b > a || a / r < b,
        _ => false,
    }
}

fn div_rounded_too_low_for_up(a: f64, b: f64, r: f64) -> bool {
    match (sign_class(a), sign_class(b)) {
        (Ordering::Greater, Ordering::Greater) => r * b < a || a / r < b,
        (Ordering::Greater, Ordering::Less) => r * b > a || a / r > b,
        (Ordering::Less, Ordering::Greater) => r * b > a || a / r < b,
        (Ordering::Less, Ordering::Less) => r * b < a || a / r > b,
        _ => false,
    }
}

#[cfg(test)]
mod directed_tests {
    use super::*;

    fn bracket_add(a: f64, b: f64, lo: f64, hi: f64) {
        let mid = a + b;
        assert!(lo <= mid, "add_down too high: {lo} > {mid}");
        assert!(hi >= mid, "add_up too low: {hi} < {mid}");
    }

    fn bracket_mul(a: f64, b: f64, lo: f64, hi: f64) {
        let mid = a * b;
        assert!(lo <= mid, "mul_down too high: {lo} > {mid}");
        assert!(hi >= mid, "mul_up too low: {hi} < {mid}");
    }

    fn bracket_div(a: f64, b: f64, lo: f64, hi: f64) {
        let mid = a / b;
        assert!(lo <= mid, "div_down too high: {lo} > {mid}");
        assert!(hi >= mid, "div_up too low: {hi} < {mid}");
    }

    #[test]
    fn add_brackets_nearest_sum() {
        let a = 1.0_f64;
        let b = 1.0_f64.next_up();
        bracket_add(a, b, f64_add_down(a, b), f64_add_up(a, b));
    }

    #[test]
    fn mul_brackets_product() {
        let a = 1.1_f64;
        let b = 1.1_f64;
        bracket_mul(a, b, f64_mul_down(a, b), f64_mul_up(a, b));
    }

    #[test]
    fn mul_negative_operand_brackets() {
        let a = -1.1_f64;
        let b = 2.3_f64;
        bracket_mul(a, b, f64_mul_down(a, b), f64_mul_up(a, b));
    }

    #[test]
    fn div_negative_divisor_brackets() {
        let a = 1.0_f64;
        let b = -3.0_f64;
        bracket_div(a, b, f64_div_down(a, b), f64_div_up(a, b));
    }

    #[test]
    fn sub_brackets_difference() {
        let a = 1.0_f64.next_up();
        let b = 1.0_f64;
        let mid = a - b;
        let lo = f64_sub_down(a, b);
        let hi = f64_sub_up(a, b);
        assert!(lo <= mid);
        assert!(hi >= mid);
    }

    #[test]
    fn exhaustive_small_add_mul() {
        let mut seed = 1u32;
        for _ in 0..5000 {
            seed = seed.wrapping_mul(1_664_525);
            let a = f64::from_bits((seed as u64) << 40);
            seed = seed.wrapping_mul(1_664_525);
            let b = f64::from_bits((seed as u64) << 40);
            if a.is_finite() && b.is_finite() {
                bracket_add(a, b, f64_add_down(a, b), f64_add_up(a, b));
                if a != 0.0 && b != 0.0 {
                    bracket_mul(a, b, f64_mul_down(a, b), f64_mul_up(a, b));
                }
                if b != 0.0 {
                    bracket_div(a, b, f64_div_down(a, b), f64_div_up(a, b));
                }
            }
        }
    }
}
