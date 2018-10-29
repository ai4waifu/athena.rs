//! Rounding policy and directed IEEE binary64 primitives for interval enclosure.

mod directed;

pub use directed::{
    f64_add_down, f64_add_up, f64_div_down, f64_div_up, f64_mul_down, f64_mul_up, f64_sub_down, f64_sub_up,
};

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

/// Round a machine real toward the given direction (identity for stored `f64` today).
pub fn directed_round(x: f64, mode: RoundingPolicy) -> f64 {
    if x.is_nan() {
        return x;
    }
    match mode {
        RoundingPolicy::NearestEven | RoundingPolicy::TowardZero => x,
        RoundingPolicy::TowardNegInf => {
            if x.is_infinite() && x.is_sign_negative() {
                x
            } else if x.is_infinite() {
                f64::MAX
            } else {
                x
            }
        }
        RoundingPolicy::TowardPosInf => {
            if x.is_infinite() && x.is_sign_positive() {
                x
            } else if x.is_infinite() {
                f64::MIN
            } else {
                x
            }
        }
    }
}
