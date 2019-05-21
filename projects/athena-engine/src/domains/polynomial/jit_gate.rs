//! 可选 JIT parity 门 — eager 为真相源，JIT 不可用时不改变语义。

use athena_types::{Diagnostic, Result};

use super::{expr::Polynomial, operations::mul_polynomial, ring_table::RingTable};

/// Parity 检查结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitParityOutcome {
    /// 未启用 JIT feature。
    EagerOnly,
    /// JIT crate 报告不可用（WASM / 无编译器 / 内核未接）。
    JitUnavailable,
    /// eager 与 JIT 结果一致。
    Matched,
    /// 不一致（须拒绝 JIT 路径）。
    Mismatch {
        /// 诊断摘要。
        detail: String,
    },
}

/// 带 parity 门的多项式乘法（eager 始终返回）。
pub fn mul_with_jit_parity(lhs: Polynomial, rhs: Polynomial, rings: &RingTable) -> Result<(Polynomial, JitParityOutcome)> {
    let eager = mul_polynomial(lhs.clone(), rhs.clone(), rings)?;
    let parity = check_mul_parity(&eager, rings);
    Ok((eager, parity))
}

fn check_mul_parity(eager: &Polynomial, _rings: &RingTable) -> JitParityOutcome {
    #[cfg(feature = "jit")]
    {
        use athena_jit::{JitAvailability, availability, polynomial_mul_parity};
        match availability() {
            JitAvailability::Disabled => JitParityOutcome::EagerOnly,
            JitAvailability::UnsupportedTarget => JitParityOutcome::JitUnavailable,
            JitAvailability::Available => {
                let summary = eager.terms.len();
                map_parity(polynomial_mul_parity(move || summary, move || None::<usize>))
            }
        }
    }
    #[cfg(not(feature = "jit"))]
    {
        let _ = eager;
        JitParityOutcome::EagerOnly
    }
}

#[cfg(feature = "jit")]
fn map_parity(outcome: athena_jit::ParityOutcome) -> JitParityOutcome {
    use athena_jit::ParityOutcome;
    match outcome {
        ParityOutcome::EagerOnly => JitParityOutcome::EagerOnly,
        ParityOutcome::JitUnavailable => JitParityOutcome::JitUnavailable,
        ParityOutcome::Matched => JitParityOutcome::Matched,
        ParityOutcome::Mismatch { detail } => JitParityOutcome::Mismatch { detail },
    }
}

/// parity 失败时返回诊断。
pub fn parity_diagnostic(outcome: &JitParityOutcome) -> Option<Diagnostic> {
    match outcome {
        JitParityOutcome::Mismatch { detail } => Some(
            athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("domain", "polynomial")
                .detail("operation", "jit_parity_mismatch")
                .detail("detail", detail.clone()),
        ),
        _ => None,
    }
}
