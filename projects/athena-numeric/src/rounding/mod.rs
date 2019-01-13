//! 舍入策略与区间包络用定向 IEEE binary64 原语。

mod directed;

pub use directed::{f64_add_down, f64_add_up, f64_div_down, f64_div_up, f64_mul_down, f64_mul_up, f64_sub_down, f64_sub_up};

/// 舍入策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingPolicy {
    /// 舍入到最近，平局取偶。
    #[default]
    NearestEven,
    /// 朝向零。
    TowardZero,
    /// 朝 +∞（区间上端点）。
    TowardPosInf,
    /// 朝 −∞（区间下端点）。
    TowardNegInf,
}

/// 将机器实数朝给定方向舍入（当前对已存 `f64` 为恒等）。
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
