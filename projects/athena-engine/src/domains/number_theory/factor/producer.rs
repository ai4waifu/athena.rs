//! 分解 producer 合同：纯 Rust 默认，可选外部 backend 占位。

use athena_numeric::Integer;

/// 外部 / 可插拔分解生产者（如未来可选 GMP-ECM）。
///
/// 合同：返回非平凡真因子，或 `None`（未找到 / 不支持）。不得 panic。
pub trait FactorProducer {
    /// 尝试从 `n` 提取一个非平凡因子。
    fn try_split(&self, n: &Integer, seed: u64, max_steps: u64) -> Option<Integer>;
}

/// 纯 Rust / WASM 可移植 fallback（空操作，pipeline 内建算法负责实际分解）。
#[derive(Debug, Copy, Clone, Default)]
pub struct PureRustFactorProducer;

impl FactorProducer for PureRustFactorProducer {
    fn try_split(&self, _n: &Integer, _seed: u64, _max_steps: u64) -> Option<Integer> {
        None
    }
}
