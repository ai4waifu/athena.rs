//! 将操作层超边排入 `OuterCandidate` 池。

use athena_ir::TermStore;

use crate::reasoning::mgraph::{admission::hyper_edge_to_outer_candidate, core::state::MGraphState};

use super::operational::OperationalState;

/// 将操作层超边排入 `OuterCandidate` 池的报告。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HyperEdgeDrainReport {
    /// 成功映射为外层候选的边。
    pub staged: u32,
    /// 仍保留在 `hyper_edges` 中的边（不支持 / 尚不可映射）。
    pub retained: u32,
}

/// 将可暂存的操作超边移入 [`OperationalState::outer_candidates`]。
///
/// 不支持的边留在 `hyper_edges`。绝不接纳进 SemanticCore / ExactUF。
pub fn drain_hyper_edges_to_outer_pool(store: &TermStore, state: &mut MGraphState) -> HyperEdgeDrainReport {
    drain_operational_hyper_edges(store, &mut state.operational)
}

fn drain_operational_hyper_edges(store: &TermStore, operational: &mut OperationalState) -> HyperEdgeDrainReport {
    let pending = std::mem::take(&mut operational.hyper_edges);
    let mut retained = Vec::new();
    let mut staged = 0u32;
    for edge in pending {
        match hyper_edge_to_outer_candidate(store, &edge) {
            Ok(outer) => {
                operational.outer_candidates.push(outer);
                staged = staged.saturating_add(1);
            }
            Err(_) => retained.push(edge),
        }
    }
    let retained_count = retained.len() as u32;
    operational.hyper_edges = retained;
    HyperEdgeDrainReport { staged, retained: retained_count }
}
