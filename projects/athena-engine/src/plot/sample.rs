//! 均匀网格 1D 采样。

use athena_types::{Diagnostic, DiagnosticCode, Number, Result};

use crate::{
    calculus::replace_symbol,
    eval::evaluate,
    term::{Term, number_from_term},
};

use super::types::{SampleDomain, SamplePoint, SampledCurve, SamplingPolicy};

/// 硬上限，防止资源耗尽。
const MAX_SAMPLES_HARD: u32 = 1_000_000;

/// 对一元表达式在实区间上均匀采样。
///
/// `expr` 中的符号 `var` 被替换为机器实数后求值；无法得到有限 `f64` 的点记为 gap。
/// 相邻有效点若相对跳跃超过 [`SamplingPolicy::discontinuity_rel`]，在后一点插入 gap（断点/奇点邻域）。
pub fn sample_1d(expr: &Term, var: &str, domain: SampleDomain, policy: SamplingPolicy) -> Result<SampledCurve> {
    if policy.is_cancelled() {
        return Err(cancelled());
    }
    if !domain.start.is_finite() || !domain.end.is_finite() || !(domain.start < domain.end) {
        return Err(Diagnostic::new(DiagnosticCode::SamplingDomainInvalid)
            .detail("domain", "plot")
            .detail("operation", "sample_1d")
            .arg("start", domain.start.to_string())
            .arg("end", domain.end.to_string()));
    }
    if policy.max_samples < 2 || policy.max_samples > MAX_SAMPLES_HARD {
        return Err(Diagnostic::new(DiagnosticCode::SamplingResourceLimit)
            .detail("domain", "plot")
            .detail("operation", "sample_1d")
            .arg("max_samples", u64::from(policy.max_samples)));
    }

    let n = policy.max_samples as usize;
    let mut curve = SampledCurve { points: Vec::with_capacity(n), gaps: Vec::new() };
    let span = domain.end - domain.start;
    let mut prev_valid_y: Option<f64> = None;
    for i in 0..n {
        if policy.is_cancelled() {
            return Err(cancelled());
        }
        let t = i as f64 / (n - 1) as f64;
        let x = domain.start + span * t;
        let substituted = replace_symbol(expr, var, &Term::number(Number::machine(x)));
        let value = evaluate(&substituted);
        let (y, valid) = match number_from_term(&value).and_then(|num| num.to_f64_lossy()) {
            Some(y) if y.is_finite() => (y, true),
            _ => (f64::NAN, false),
        };
        let mut gap_here = !valid;
        if valid {
            if let (Some(prev), Some(rel)) = (prev_valid_y, policy.discontinuity_rel) {
                if is_discontinuity_jump(prev, y, rel) {
                    gap_here = true;
                }
            }
            prev_valid_y = Some(y);
        }
        else {
            prev_valid_y = None;
        }
        if gap_here {
            curve.gaps.push(curve.points.len());
        }
        curve.points.push(SamplePoint { x, y, valid });
    }
    Ok(curve)
}

fn cancelled() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::SamplingCancelled).detail("domain", "plot").detail("operation", "sample_1d")
}

fn is_discontinuity_jump(prev: f64, y: f64, rel: f64) -> bool {
    let scale = 1.0 + prev.abs() + y.abs();
    if (y - prev).abs() > rel * scale {
        return true;
    }
    // 渐近线两侧：异号且 |y| 均较大，禁止连线跨极点。
    prev.signum() != y.signum() && prev.abs() > 4.0 && y.abs() > 4.0
}
