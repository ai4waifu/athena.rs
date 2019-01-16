//! 区间包络用的定向 IEEE binary64 舍入。
//!
//! 外向舍入用无误差反向检查（Boost.Interval 风格），而非再舍入的反向运算。

use std::cmp::Ordering;

/// `a + b` 朝 −∞ 舍入。
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

/// `a + b` 朝 +∞ 舍入。
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

/// `a - b` 朝 −∞ 舍入。
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

/// `a - b` 朝 +∞ 舍入。
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

/// `a * b` 朝 −∞ 舍入。
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

/// `a * b` 朝 +∞ 舍入。
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

/// `a / b` 朝 −∞ 舍入。
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

/// `a / b` 朝 +∞ 舍入。
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
