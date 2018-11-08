//! 域注册表：FieldId 与 FieldPresentation（Phase 0 骨架）。

use std::collections::HashMap;

use athena_numeric::Integer;
use athena_types::{Diagnostic, DiagnosticCode, FieldId, PresentationId};

use crate::number_theory::{Primality, primality_test};

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

    /// 注册 Q。
    pub fn rationals(&mut self) -> FieldId {
        self.intern(FieldInternKey::Rationals, FieldPresentationKind::Rationals)
    }

    /// 注册素域 F_p（p 须为已验证素数）。
    pub fn prime_field(&mut self, characteristic: Integer) -> Result<FieldId, Diagnostic> {
        validate_prime_modulus(&characteristic)?;
        Ok(self.intern(
            FieldInternKey::Prime { characteristic: characteristic.clone() },
            FieldPresentationKind::PrimeField { characteristic },
        ))
    }

    /// 按 FieldId 查 presentation。
    pub fn presentation(&self, field: FieldId) -> Option<&FieldPresentation> {
        self.field_to_presentation.get(&field).and_then(|id| self.presentations.get(id))
    }

    /// 域特征（素域返回 p；其他表示 Phase 2+ 填充）。
    pub fn characteristic(&self, field: FieldId) -> Option<Integer> {
        match self.presentation(field).map(|p| &p.kind) {
            Some(FieldPresentationKind::PrimeField { characteristic }) => Some(characteristic.clone()),
            _ => None,
        }
    }

    /// 校验 FieldId 已注册且 presentation 支持系数约化。
    pub fn validate_finite_field(&self, field: FieldId) -> Result<(), Diagnostic> {
        let pres = self.presentation(field).ok_or_else(|| unknown_field(field))?;
        match &pres.kind {
            FieldPresentationKind::PrimeField { .. } => Ok(()),
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "field")
                .detail("operation", "coeff_presentation_unsupported")),
        }
    }

    /// 素域 𝔽_p 的约化模数（经 presentation 查找，Phase 3 系数内核真相源）。
    pub fn prime_modulus(&self, field: FieldId) -> Result<athena_numeric::Modulus, Diagnostic> {
        let p = self.characteristic(field).ok_or_else(|| unknown_field(field))?;
        athena_numeric::Modulus::new(p).map_err(|_| {
            Diagnostic::new(DiagnosticCode::ModulusInvalid)
                .detail("domain", "field")
                .detail("operation", "prime_modulus_from_presentation")
        })
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

fn validate_prime_modulus(p: &Integer) -> Result<(), Diagnostic> {
    if p.is_zero() || p.is_negative() {
        return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
            .detail("domain", "field")
            .detail("operation", "prime_field_characteristic"));
    }
    match primality_test(p, None) {
        Primality::Prime => Ok(()),
        Primality::Composite => Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
            .detail("domain", "field")
            .detail("operation", "prime_field_not_prime")),
        Primality::ProbablePrime { .. } | Primality::Unknown => Err(Diagnostic::new(DiagnosticCode::PrimeTestInconclusive)
            .detail("domain", "field")
            .detail("operation", "prime_field_characteristic")),
    }
}

fn unknown_field(field: FieldId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "field")
        .detail("operation", "unknown_field")
        .detail("field_id", field.0.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_prime_field_rejected() {
        let mut table = FieldTable::new();
        let err = table.prime_field(Integer::from_i64(6)).unwrap_err();
        assert_eq!(err.code.as_str(), "ATHENA_MODULUS_INVALID");
    }

    #[test]
    fn prime_field_intern_idempotent() {
        let mut table = FieldTable::new();
        let a = table.prime_field(Integer::from_i64(5)).unwrap();
        let b = table.prime_field(Integer::from_i64(5)).unwrap();
        assert_eq!(a, b);
        assert_eq!(table.characteristic(a), Some(Integer::from_i64(5)));
    }
}
