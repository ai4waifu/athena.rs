//! ANV1 / wire 规范校验（非规范编码应拒绝，而非静默归一化）。
//!
//! 具体拒绝规则在 decoder 路径逐步落地；本模块保留校验入口。

use athena_types::{Diagnostic, DiagnosticCode, Result};

/// 拒绝非规范零 / 高位零 / 非法符号等（骨架；完整规则随后续 wire 门补齐）。
pub fn reject_non_canonical_reason(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
        .detail("domain", "numeric")
        .detail("wire", "ANV1")
        .detail("reason", reason)
}

/// 占位：当前多数 decoder 仍在 `format::binary` 内联校验。
pub fn assert_canonical_placeholder() -> Result<()> {
    Ok(())
}
