//! 多项式理想（生成元列表 + 环身份）。

use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::expr::Polynomial;

/// 多项式理想 ⟨generators⟩ ⊂ R[x]。
#[derive(Debug, Clone, PartialEq)]
pub struct Ideal {
    /// 所属环。
    pub ring: RingId,
    /// 生成元（须同环；不必 Gröbner / canonical）。
    pub generators: Vec<Polynomial>,
}

impl Ideal {
    /// 由生成元构造（校验同环）。
    pub fn new(generators: Vec<Polynomial>) -> Result<Self> {
        if generators.is_empty() {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "polynomial")
                .detail("operation", "ideal_empty_generators"));
        }
        let ring = generators[0].ring();
        for g in &generators[1..] {
            if g.ring() != ring {
                return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                    .detail("domain", "polynomial")
                    .detail("operation", "ideal_ring_mismatch"));
            }
        }
        Ok(Self { ring, generators })
    }
}
