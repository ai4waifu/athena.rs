//! 域注册表：FieldId 与 FieldPresentation（Phase 0 骨架）。

use std::collections::HashMap;

use athena_numeric::Integer;
use athena_types::{AlgebraMapId, Diagnostic, DiagnosticCode, ExtensionId, FieldId, PresentationId, Result};

use crate::{
    algebra::{
        finite_field_poly::{
            FiniteFieldPolySpec, canonicalize_modulus, is_irreducible_monic, validate_modulus_shape,
        },
        property::{PropertyState, PropertyWitness},
    },
    field::{Field, FieldDescriptor},
    number_theory::{Primality, primality_test},
};

use super::{
    map_table::MapTable,
    presentation::{FieldPresentation, FieldPresentationKind},
};

/// 域 intern 键（descriptor 级，不含可变算法状态）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FieldInternKey {
    Rationals,
    Prime { characteristic: Integer },
    PolynomialBasis {
        characteristic: Integer,
        modulus: Vec<Integer>,
    },
}

/// Session 级域与 presentation 注册表。
#[derive(Debug, Default)]
pub struct FieldTable {
    next_field_id: u32,
    next_presentation_id: u32,
    presentations: HashMap<PresentationId, FieldPresentation>,
    field_to_presentation: HashMap<FieldId, PresentationId>,
    by_key: HashMap<FieldInternKey, FieldId>,
    map_table: MapTable,
    poly_extensions: HashMap<FieldId, FiniteFieldPolySpec>,
    next_extension_id: u32,
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
    pub fn prime_field(&mut self, characteristic: Integer) -> Result<FieldId> {
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

    /// 域特征（素域与 𝔽_{p^n} 均返回 p）。
    pub fn characteristic(&self, field: FieldId) -> Option<Integer> {
        match self.presentation(field).map(|p| &p.kind) {
            Some(FieldPresentationKind::PrimeField { characteristic }) => Some(characteristic.clone()),
            Some(FieldPresentationKind::FiniteFieldPolynomialBasis { .. }) => {
                self.poly_extensions.get(&field).map(|s| s.characteristic.clone())
            }
            _ => None,
        }
    }

    /// 𝔽_{p^n} 多项式基规格（若已注册）。
    pub fn finite_field_poly_spec(&self, field: FieldId) -> Option<&FiniteFieldPolySpec> {
        self.poly_extensions.get(&field)
    }

    /// 注册 𝔽_{p^n}（首一不可约模多项式 + 多项式基 presentation）。
    pub fn polynomial_basis_field(&mut self, characteristic: Integer, modulus: Vec<Integer>) -> Result<FieldId> {
        validate_prime_modulus(&characteristic)?;
        let p = athena_numeric::Modulus::new(characteristic.clone())?;
        let modulus = canonicalize_modulus(modulus, &p)?;
        let degree = validate_modulus_shape(&modulus, &p)?;
        if !is_irreducible_monic(&modulus, &p)? {
            return Err(Diagnostic::new(DiagnosticCode::FieldModulusReducible)
                .detail("domain", "field")
                .detail("operation", "polynomial_basis_modulus"));
        }
        let key = FieldInternKey::PolynomialBasis { characteristic: characteristic.clone(), modulus: modulus.clone() };
        if let Some(&id) = self.by_key.get(&key) {
            return Ok(id);
        }
        let base = self.prime_field(characteristic.clone())?;
        let extension = ExtensionId(self.next_extension_id);
        self.next_extension_id = self.next_extension_id.wrapping_add(1);
        let field = FieldId(self.next_field_id);
        self.next_field_id = self.next_field_id.wrapping_add(1);
        let presentation_id = PresentationId(self.next_presentation_id);
        self.next_presentation_id = self.next_presentation_id.wrapping_add(1);
        let kind = FieldPresentationKind::FiniteFieldPolynomialBasis { field, degree };
        let presentation = FieldPresentation { id: presentation_id, field, kind };
        self.by_key.insert(key, field);
        self.field_to_presentation.insert(field, presentation_id);
        self.presentations.insert(presentation_id, presentation);
        self.poly_extensions.insert(
            field,
            FiniteFieldPolySpec { extension, base, characteristic, degree, modulus },
        );
        Ok(field)
    }

    /// 校验 FieldId 已注册且 presentation 支持系数约化。
    pub fn validate_finite_field(&self, field: FieldId) -> Result<()> {
        let pres = self.presentation(field).ok_or_else(|| unknown_field(field))?;
        match &pres.kind {
            FieldPresentationKind::PrimeField { .. } | FieldPresentationKind::FiniteFieldPolynomialBasis { .. } => Ok(()),
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "field")
                .detail("operation", "coeff_presentation_unsupported")),
        }
    }

    /// 素域 𝔽_p 的约化模数（经 presentation 查找，Phase 3 系数内核真相源）。
    pub fn prime_modulus(&self, field: FieldId) -> Result<athena_numeric::Modulus> {
        let p = self.characteristic(field).ok_or_else(|| unknown_field(field))?;
        athena_numeric::Modulus::new(p).map_err(|_| {
            Diagnostic::new(DiagnosticCode::ModulusInvalid)
                .detail("domain", "field")
                .detail("operation", "prime_modulus_from_presentation")
        })
    }

    /// 已注册 ℚ 的 [`FieldId`]（若尚未 intern 则 `None`）。
    pub fn rationals_field(&self) -> Option<FieldId> {
        self.by_key.get(&FieldInternKey::Rationals).copied()
    }

    /// 域的默认 presentation id。
    pub fn presentation_id(&self, field: FieldId) -> Result<PresentationId> {
        self.field_to_presentation.get(&field).copied().ok_or_else(|| unknown_field(field))
    }

    /// 域数学描述（不含可变算法状态）。
    pub fn descriptor(&self, field: FieldId) -> Result<FieldDescriptor> {
        let pres = self.presentation(field).ok_or_else(|| unknown_field(field))?;
        match &pres.kind {
            FieldPresentationKind::Rationals => Ok(FieldDescriptor::Rationals),
            FieldPresentationKind::PrimeField { characteristic } => {
                Ok(FieldDescriptor::Prime { characteristic: characteristic.clone() })
            }
            FieldPresentationKind::FiniteFieldPolynomialBasis { degree, .. } => {
                let spec = self.poly_extensions.get(&field).ok_or_else(|| unknown_field(field))?;
                Ok(FieldDescriptor::Extension {
                    base: spec.base,
                    extension: spec.extension,
                    degree: PropertyState::Proven {
                        value: *degree,
                        witness: PropertyWitness::placeholder("polynomial_basis"),
                    },
                })
            }
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "field")
                .detail("operation", "field_descriptor_unsupported")),
        }
    }

    /// 组装域对象（id + descriptor + presentation）。
    pub fn field_record(&self, field: FieldId) -> Result<Field> {
        Ok(Field { id: field, descriptor: self.descriptor(field)?, presentation: self.presentation_id(field)? })
    }

    /// 映射表（只读）。
    pub fn map_table(&self) -> &MapTable {
        &self.map_table
    }

    /// 映射表（可变）。
    pub fn map_table_mut(&mut self) -> &mut MapTable {
        &mut self.map_table
    }

    /// 注册 ℚ → 𝔽_p canonical embedding（显式、幂等）。
    pub fn canonical_embedding_rationals_to_prime(&mut self, target: FieldId) -> Result<AlgebraMapId> {
        self.validate_finite_field(target)?;
        let source = self.rationals_field().unwrap_or_else(|| self.rationals());
        let source_pres = self.presentation_id(source)?;
        let target_pres = self.presentation_id(target)?;
        Ok(self.map_table.register_canonical_q_to_fp(source, target, source_pres, target_pres))
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

fn validate_prime_modulus(p: &Integer) -> Result<()> {
    if p.is_zero() || p.is_negative() {
        return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
            .detail("domain", "field")
            .detail("operation", "prime_field_characteristic"));
    }
    match primality_test(p, None) {
        Primality::Prime { .. } => Ok(()),
        Primality::Composite { .. } => Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
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
