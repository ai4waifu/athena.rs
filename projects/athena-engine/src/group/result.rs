//! 群论域分派（骨架）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{request::GroupRequest, value::GroupDomainValue};

/// 群论域结果。
#[derive(Debug, Clone, PartialEq)]
pub enum GroupResult {
    /// 精确结果。
    Exact {
        /// 值。
        value: GroupDomainValue,
    },
    /// 未求值。
    Unevaluated {
        /// 原因。
        reason: Diagnostic,
    },
}

/// 执行群论域请求。
pub fn execute_group(request: GroupRequest) -> GroupResult {
    let op = match &request {
        GroupRequest::Cyclic { .. } => "cyclic",
        GroupRequest::PermutationGroup { .. } => "permutation_group",
        GroupRequest::Order { .. } => "order",
        GroupRequest::Multiply { .. } => "multiply",
        GroupRequest::Inverse { .. } => "inverse",
        GroupRequest::IsAbelian { .. } => "is_abelian",
    };
    GroupResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "group").detail("operation", op),
    }
}
