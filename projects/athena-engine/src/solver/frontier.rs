//! Frontier 评分（骨架）。

use crate::mgraph::{SolverCandidate, SolverScore};

/// 朴素评分：固定占位。
pub fn score_candidate(_candidate: &SolverCandidate) -> SolverScore {
    SolverScore { estimated_benefit: 1.0, estimated_cost: 1.0, confidence: 0.5, unlocks: 0 }
}
