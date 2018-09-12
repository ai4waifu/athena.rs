//! 代数整数相关入口（骨架）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::result::NumberTheoryResult;

/// 代数整数判定 / 范数等（骨架）。
pub fn algebraic_scaffold() -> NumberTheoryResult {
    NumberTheoryResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "number_theory")
            .detail("operation", "algebraic"),
    }
}
