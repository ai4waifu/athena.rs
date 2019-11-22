//! Budget and stop reasons for scope-local saturation (Living `03` R-2.5).

/// Hard caps for one saturation run. Zero means “disabled / immediate stop”.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaturationBudget {
    /// Maximum equality-class count.
    pub max_eclasses: u32,
    /// Maximum enode count.
    pub max_enodes: u32,
    /// Maximum rewrite / merge iterations.
    pub max_iterations: u32,
    /// Maximum candidate unions emitted this run.
    pub max_candidate_unions: u32,
}

impl Default for SaturationBudget {
    fn default() -> Self {
        Self {
            max_eclasses: 1_024,
            max_enodes: 4_096,
            max_iterations: 64,
            max_candidate_unions: 512,
        }
    }
}

impl SaturationBudget {
    /// Tiny budget for smoke / contract tests.
    pub const fn smoke() -> Self {
        Self {
            max_eclasses: 32,
            max_enodes: 128,
            max_iterations: 8,
            max_candidate_unions: 16,
        }
    }
}

/// Why saturation stopped (never “ran forever”).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturationStopReason {
    /// Fixed point within budget (no pending work).
    FixedPoint,
    /// Hit iteration cap.
    IterationBudget,
    /// Hit eclass / enode / union caps.
    ResourceBudget,
    /// Caller cancelled (future hook).
    Cancelled,
}
