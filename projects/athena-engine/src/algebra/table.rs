//! 域注册表：FieldId ↔ FieldPresentation（Phase 0 骨架）。

use std::collections::HashMap;

use athena_numeric::Integer;
use athena_types::{Diagnostic, DiagnosticCode, FieldId, PresentationId};

use super::presentation::{FieldPresentation, FieldPresentationKind};

/// 域 intern 键（descriptor 级，不含可变算法状态）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FieldInternKey {
    Rationals,
    Prime { characteristic: Integer },
}

/// Session 级域与 presentation 注册表。
#[derive(Debug, Default)]
pub struct FieldTable {
    next_field_id: u32,
    next_presentation_id: u32,
    presentations: HashMap<PresentationId, FieldPresentation>,
    field_to_presentation: HashMap<FieldId, PresentationId>,
    by_key: HashMap<FieldInternKey, FieldId>,
}

impl FieldTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册 ℚ。
    pub fn rationals(&mut self) -> FieldId {
        self.intern(FieldInternKey::Rationals, FieldPresentationKind::Rationals)
    }

    /// 注册素域 𝔽_p（p 须为正；素性证明后续）。
    pub fn prime_field(&mut self, characteristic: Integer) -> Result<FieldId, Diagnostic> {
        if characteristic.is_zero() || characteristic.is_negative() {
            return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
                .detail("domain", "field")
                .detail("operation", "prime_field"));
        }
        Ok(self.intern(
            FieldInternKey::Prime { characteristic: characteristic.clone() },
            FieldPresentationKind::PrimeField { characteristic },
        ))
    }

    /// 按 FieldId 查 presentation。
    pub fn presentation(&self, field: FieldId) -> Option<&FieldPresentation> {
        self.field_to_presentation.get(&field).and_then(|id| self.presentations.get(id))
    }

    fn intern(&mut self, key: FieldInternKey, kind: FieldPresentationKind) -> FieldId {
        if let Some(&id) = self.by_key.get(&key) {
            return id;
        }
        let field = FieldId(self.next_field_id);
        self.next_field_id = self.next_field_id.wrapping_add(1);
        let presentation_id = PresentationId(self.next_presentation_id);
        self.next_presentation_id = self.next_presentation_id.wrapping_add(1);
        let presentation = FieldPresentation { id: presentation_id, field, kind };
        self.by_key.insert(key, field);
        self.field_to_presentation.insert(field, presentation_id);
        self.presentations.insert(presentation_id, presentation);
        field
    }
}
