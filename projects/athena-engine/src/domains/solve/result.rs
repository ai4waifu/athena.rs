//! Solve 域结果。

use athena_types::{Diagnostic, TermId};

use super::coverage::CoverageStatus;

/// Solve 执行结果（投影为规则列表项，或未求值残差）。
#[derive(Debug, PartialEq)]
pub enum SolveResult {
    /// 解集已物化为 `{{x -> …}, …}` 风格嵌套列表（含空解 `{}`）。
    Exact {
        /// 物化规则列表项。
        term: TermId,
        /// 解集覆盖承诺（不得在结果层丢弃）。
        coverage: CoverageStatus,
    },
    /// 未求值（保留 `Solve[…]` 表面或方程回声）。
    Unevaluated {
        /// 残差表达式。
        expression: TermId,
        /// 原因。
        reason: Diagnostic,
    },
}
