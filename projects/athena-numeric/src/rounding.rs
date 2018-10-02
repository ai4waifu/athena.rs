//! Rounding policy and directed `f64` primitives for interval enclosure.

/// Rounding policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingPolicy {
    /// Round to nearest, ties to even.
    #[default]
    NearestEven,
    /// Toward zero.
    TowardZero,
    /// Toward +∞ (upper interval endpoints).
    TowardPosInf,
    /// Toward −∞ (lower interval endpoints).
    TowardNegInf,
}

/// Round a machine real toward the given direction (no-op for values already stored as `f64`).
pub fn directed_round(x: f64, mode: RoundingPolicy) -> f64 {
    if x.is_nan() {
        return x;
    }
    match mode {
        RoundingPolicy::NearestEven | RoundingPolicy::TowardZero => x,
        RoundingPolicy::TowardNegInf => {
            if x.is_infinite() && x.is_sign_negative() {
                x
            }
            else if x.is_infinite() {
                f64::MAX
            }
            else {
                x
            }
        }
        RoundingPolicy::TowardPosInf => {
            if x.is_infinite() && x.is_sign_positive() {
                x
            }
            else if x.is_infinite() {
                f64::MIN
            }
            else {
                x
            }
        }
    }
}

/// `a + b` rounded toward −∞.
pub fn f64_add_down(a: f64, b: f64) -> f64 {
    let z = a + b;
    if !z.is_finite() {
        return z;
    }
    let down = z.next_down();
    if down + b <= a { down } else { z }
}

/// `a + b` rounded toward +∞.
pub fn f64_add_up(a: f64, b: f64) -> f64 {
    let z = a + b;
    if !z.is_finite() {
        return z;
    }
    let up = z.next_up();
    if a <= up - b { up } else { z }
}

/// `a - b` rounded toward −∞.
pub fn f64_sub_down(a: f64, b: f64) -> f64 {
    f64_add_down(a, -b)
}

/// `a - b` rounded toward +∞.
pub fn f64_sub_up(a: f64, b: f64) -> f64 {
    f64_add_up(a, -b)
}

/// `a * b` rounded toward −∞.
pub fn f64_mul_down(a: f64, b: f64) -> f64 {
    let z = a * b;
    if !z.is_finite() || z == 0.0 {
        return z;
    }
    let down = z.next_down();
    if down * b <= a * b { down } else { z }
}

/// `a * b` rounded toward +∞.
pub fn f64_mul_up(a: f64, b: f64) -> f64 {
    let z = a * b;
    if !z.is_finite() || z == 0.0 {
        return z;
    }
    let up = z.next_up();
    if up * b >= a * b { up } else { z }
}

/// `a / b` rounded toward −∞.
pub fn f64_div_down(a: f64, b: f64) -> f64 {
    let z = a / b;
    if !z.is_finite() {
        return z;
    }
    let down = z.next_down();
    if down * b <= a { down } else { z }
}

/// `a / b` rounded toward +∞.
pub fn f64_div_up(a: f64, b: f64) -> f64 {
    let z = a / b;
    if !z.is_finite() {
        return z;
    }
    let up = z.next_up();
    if a <= up * b { up } else { z }
}
