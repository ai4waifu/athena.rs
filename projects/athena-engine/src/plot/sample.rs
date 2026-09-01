//! 均匀网格 1D 采样。

use athena_types::{Diagnostic, DiagnosticCode, Number, Result};

use crate::{
    calculus::replace_symbol,
    eval::evaluate,
    term::{Term, number_from_term},
};

use super::types::{SampleDomain, SamplePoint, SampledCurve, SamplingPolicy};

/// 对一元表达式在实区间上均匀采样。
///
/// `expr` 中的符号 `var` 被替换为机器实数后求值；无法得到有限 `f64` 的点记为 gap。
pub fn sample_1d(expr: &Term, var: &str, domain: SampleDomain, policy: SamplingPolicy) -> Result<SampledCurve> {
    if !domain.start.is_finite() || !domain.end.is_finite() || !(domain.start < domain.end) {
        return Err(Diagnostic::new(DiagnosticCode::SamplingDomainInvalid)
            .detail("domain", "plot")
            .detail("operation", "sample_1d")
            .arg("start", domain.start.to_string())
            .arg("end", domain.end.to_string()));
    }
    if policy.max_samples < 2 {
        return Err(Diagnostic::new(DiagnosticCode::SamplingResourceLimit)
            .detail("domain", "plot")
            .detail("operation", "sample_1d")
            .arg("max_samples", u64::from(policy.max_samples)));
    }

    let n = policy.max_samples as usize;
    let mut curve = SampledCurve {
        points: Vec::with_capacity(n),
        gaps: Vec::new(),
    };
    let span = domain.end - domain.start;
    for i in 0..n {
        let t = i as f64 / (n - 1) as f64;
        let x = domain.start + span * t;
        let substituted = replace_symbol(expr, var, &Term::number(Number::machine(x)));
        let value = evaluate(&substituted);
        let (y, valid) = match number_from_term(&value).and_then(|num| num.to_f64_lossy()) {
            Some(y) if y.is_finite() => (y, true),
            _ => (f64::NAN, false),
        };
        if !valid {
            curve.gaps.push(curve.points.len());
        }
        curve.points.push(SamplePoint { x, y, valid });
    }
    Ok(curve)
}
