//! GC 分配错误 → 诊断。

use athena_gc::GcError;
use athena_types::{Diagnostic, DiagnosticCode};

/// 将 `athena-gc` 错误映射为 numeric 诊断。
pub(crate) fn gc_alloc_err(err: GcError) -> Diagnostic {
    match err {
        GcError::ArenaBytesLimit { requested_total, limit }
        | GcError::ScratchBytesLimit { requested_total, limit } => {
            Diagnostic::new(DiagnosticCode::NumericResourceLimit)
                .detail("domain", "numeric")
                .detail("kind", "arena_bytes")
                .detail("got", requested_total.to_string())
                .detail("max", limit.to_string())
        }
        GcError::LimbLimit { requested, limit } => Diagnostic::new(DiagnosticCode::NumericResourceLimit)
            .detail("domain", "numeric")
            .detail("kind", "limbs")
            .detail("got", requested.to_string())
            .detail("max", limit.to_string()),
        GcError::SegmentCountLimit { count, limit } => Diagnostic::new(DiagnosticCode::NumericResourceLimit)
            .detail("domain", "numeric")
            .detail("kind", "segments")
            .detail("got", count.to_string())
            .detail("max", limit.to_string()),
        other => Diagnostic::new(DiagnosticCode::NumericResourceLimit)
            .detail("domain", "numeric")
            .detail("kind", "gc")
            .detail("reason", other.to_string()),
    }
}
