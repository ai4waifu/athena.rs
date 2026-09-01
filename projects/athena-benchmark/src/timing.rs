//! 计时：warmup + 采样 → p50 / p95。

use std::time::{Duration, Instant};

use serde::Serialize;

/// 纳秒级分位数摘要。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TimingStats {
    /// 中位耗时（纳秒）。
    pub p50_ns: u64,
    /// 95 分位耗时（纳秒）。
    pub p95_ns: u64,
}

/// 对 `f` 做 `warmup` 次预热后采样 `samples` 次，返回分位数。
pub fn measure(warmup: usize, samples: usize, mut f: impl FnMut()) -> TimingStats {
    for _ in 0..warmup {
        f();
    }
    let mut times = Vec::with_capacity(samples.max(1));
    for _ in 0..samples.max(1) {
        let start = Instant::now();
        f();
        times.push(duration_ns(start.elapsed()));
    }
    times.sort_unstable();
    TimingStats {
        p50_ns: percentile(&times, 50),
        p95_ns: percentile(&times, 95),
    }
}

fn duration_ns(d: Duration) -> u64 {
    d.as_nanos().min(u128::from(u64::MAX)) as u64
}

fn percentile(sorted: &[u64], pct: u8) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (f64::from(pct) / 100.0) * (sorted.len() as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let w = rank - lo as f64;
        let a = sorted[lo] as f64;
        let b = sorted[hi] as f64;
        (a + (b - a) * w).round() as u64
    }
}
