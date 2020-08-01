//! Solve 域结果。

use athena_types::{Diagnostic, TermId};

/// Solve 执行结果（投影为规则列表项，或未求值残差）。
#[derive(Debug, PartialEq)]
pub enum SolveResult {
    /// 精确解集已物化为 `{{x -> …}, …}` 风格嵌套列表。
    Exact {
        /// 物化规则列表项。
        term: TermId,
    },
    /// 未求值（保留 `Solve[…]` 表面或方程回声）。
    Unevaluated {
        /// 残差表达式。
        expression: TermId,
        /// 原因。
        reason: Diagnostic,
    },
}
