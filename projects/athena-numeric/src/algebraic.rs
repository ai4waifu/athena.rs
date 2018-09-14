//! 代数数骨架。

use athena_types::TermId;

use crate::interval::Interval;

/// 代数数表示策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlgebraicRepresentation {
    /// 极小多项式 + 隔离区间（多项式对象后续接 engine）。
    MinimalPolynomial {
        /// 极小多项式 IR 引用（骨架）。
        polynomial: TermId,
        /// 根下标。
        root_index: u32,
    },
    /// 占位。
    Placeholder,
}

/// 代数数。
#[derive(Debug, Clone, PartialEq)]
pub struct AlgebraicNumber {
    /// 极小多项式引用（骨架用 `TermId`）。
    pub minimal_polynomial: TermId,
    /// 隔离区间。
    pub isolating_interval: Interval,
    /// 表示。
    pub representation: AlgebraicRepresentation,
}
