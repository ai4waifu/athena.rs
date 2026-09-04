//! 公共诊断构造与表达式摘要。

pub mod expression_summary;

use athena_types::{Diagnostic, DiagnosticCode};

/// 构造 `UnsupportedOperation` 诊断。
pub fn unsupported_operation(operation: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", operation)
}

/// 构造非法下标诊断。
pub fn invalid_index_diagnostic(index: i64, length: Option<u64>) -> Diagnostic {
    let d = Diagnostic::new(DiagnosticCode::InvalidIndex).arg("index", index);
    match length {
        Some(len) => d.arg("length", len),
        None => d,
    }
}

/// 构造非布尔条件诊断。
pub fn non_boolean_condition_diagnostic(got: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NonBooleanCondition).detail("expected", "Boolean").detail("got", got)
}
