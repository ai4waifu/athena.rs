//! Frontier 评分（骨架）。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::mgraph::{SolverCandidate, SolverScore};

/// 朴素评分：由占位浮点估计量化为稳定整数。
pub fn score_candidate(candidate: &SolverCandidate) -> SolverScore {
    let estimated_benefit = 1.0_f64;
    let estimated_cost = 1.0_f64;
    let confidence = 0.5_f64;
    let unlocks = 0_usize;
    let total = quantize_score(estimated_benefit, estimated_cost, confidence, unlocks);
    SolverScore {
        total,
        tie_breaker: stable_tie_breaker(candidate),
    }
}

fn quantize_score(benefit: f64, cost: f64, confidence: f64, unlocks: usize) -> i64 {
    let benefit = if benefit.is_finite() && benefit >= 0.0 { benefit } else { 0.0 };
    let cost = if cost.is_finite() && cost > 0.0 { cost } else { 1.0 };
    let confidence = if confidence.is_finite() {
        confidence.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let ratio = (benefit / cost) * (1.0 + confidence);
    let unlock_bonus = (unlocks as f64).min(1_000.0);
    ((ratio + unlock_bonus) * 1_000.0).round() as i64
}

fn stable_tie_breaker(candidate: &SolverCandidate) -> u64 {
    let mut h = DefaultHasher::new();
    candidate.solver.0.hash(&mut h);
    for root in &candidate.roots {
        root.0.hash(&mut h);
    }
    h.finish()
}
