//! Proof forest for admitted equalities (Living `03` R-2.4 · bootstrap).
//!
//! Records *why* two terms are equal after AdmissionGate. Distinct from
//! scope-local E-Graph candidate unions and from operational [`ExactUnionFind`].

use athena_types::TermId;

/// One justified equality edge in the forest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofEdge {
    /// Left term.
    pub left: TermId,
    /// Right term.
    pub right: TermId,
    /// Opaque step kind (filled by verifiers later).
    pub step_kind: ProofStepKind,
}

/// Closed step taxonomy for bootstrap (expand with certificates later).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofStepKind {
    /// Direct admitted equality (structural / harness / generic).
    AdmittedEquality,
    /// Congruence under a common head (ExactUF application congruence).
    Congruence,
    /// Typed rewrite replay (`match_pattern` + `substitute`).
    TypedRewrite,
    /// Transitivity step.
    Transitivity,
}

/// Forest of equality justifications (append-only bootstrap).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProofForest {
    edges: Vec<ProofEdge>,
}

impl ProofForest {
    /// Empty forest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a justified equality (does not itself admit M-Graph facts).
    pub fn record(&mut self, left: TermId, right: TermId, step_kind: ProofStepKind) {
        self.edges.push(ProofEdge { left, right, step_kind });
    }

    /// Edge count.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Iterate edges.
    pub fn edges(&self) -> &[ProofEdge] {
        &self.edges
    }
}

#[cfg(test)]
mod tests {
    use athena_types::TermId;

    use super::*;

    #[test]
    fn proof_forest_records_admitted_equality_edges() {
        let mut forest = ProofForest::new();
        forest.record(TermId(1), TermId(2), ProofStepKind::AdmittedEquality);
        assert_eq!(forest.len(), 1);
        assert_eq!(forest.edges()[0].step_kind, ProofStepKind::AdmittedEquality);
    }
}
