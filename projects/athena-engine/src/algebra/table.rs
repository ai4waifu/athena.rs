//! 域注册表：`FieldId` 与 `FieldPresentation` 的 intern 与查找。

use std::collections::HashMap;

use athena_numeric::{Integer, Rational};
use athena_types::{
    AlgebraMapId, AutomorphismId, Diagnostic, DiagnosticCode, ExtensionId, FieldId, FieldPresentationId, Result,
};

use crate::{
    algebra::{
        finite_field_poly::{FiniteFieldPolySpec, canonicalize_modulus, is_irreducible_monic, validate_modulus_shape},
        number_field::{
            NumberFieldSpec, absolute_degree_product, is_irreducible_over_rationals, make_monic,
            relative_modulus_from_rational, validate_rational_modulus,
        },
        property::{PropertyState, PropertyWitness},
    },
    field::{Field, FieldDescriptor},
    number_theory::{Primality, primality_test},
};

use super::{
    extension::{FieldExtension, extension_tower_fields},
    fingerprint::FieldFingerprint,
    map_table::MapTable,
    presentation::{FieldPresentation, FieldPresentationKind},
};

/// 域 intern 键（descriptor 级，不含可变算法状态）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FieldInternKey {
    Rationals,
    Prime { characteristic: Integer },
    PolynomialBasis { characteristic: Integer, modulus: Vec<Integer> },
    NumberField { absolute_modulus: Vec<(Integer, Integer)> },
}

/// Session 级域与 presentation 注册表。
#[derive(Debug, Default)]
pub struct FieldTable {
    next_field_id: u32,
    next_presentation_id: u32,
    presentations: HashMap<FieldPresentationId, FieldPresentation>,
    field_to_presentation: HashMap<FieldId, FieldPresentationId>,
    by_key: HashMap<FieldInternKey, FieldId>,
    map_table: MapTable,
    poly_extensions: HashMap<FieldId, FiniteFieldPolySpec>,
    number_fields: HashMap<FieldId, NumberFieldSpec>,
    extensions: HashMap<ExtensionId, FieldExtension>,
    field_to_extension: HashMap<FieldId, ExtensionId>,
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

    /// 域内容指纹（跨 Session 可比较；不含 [`FieldId`]）。
    pub fn field_fingerprint(&self, field: FieldId) -> Option<FieldFingerprint> {
        match self.presentation(field).map(|p| &p.kind)? {
            FieldPresentationKind::Rationals => Some(FieldFingerprint::rationals()),
            FieldPresentationKind::PrimeField { characteristic } => Some(FieldFingerprint::prime_field(characteristic)),
            FieldPresentationKind::FiniteFieldPolynomialBasis { .. } => {
                let spec = self.poly_extensions.get(&field)?;
                Some(FieldFingerprint::finite_field_polynomial_basis(&spec.characteristic, &spec.modulus))
            }
            FieldPresentationKind::NumberFieldPowerBasis { .. } | FieldPresentationKind::NumberFieldTower { .. } => {
                let spec = self.number_fields.get(&field)?;
                let abs: Vec<_> =
                    spec.absolute_modulus.iter().map(|c| (c.numerator().clone(), c.denominator().clone())).collect();
                Some(FieldFingerprint::number_field(&abs))
            }
            other => Some(FieldFingerprint::from_presentation_kind_tag(other)),
        }
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

    /// 按扩张 id 查 [`FieldExtension`]。
    pub fn extension_record(&self, extension: ExtensionId) -> Option<&FieldExtension> {
        self.extensions.get(&extension)
    }

    /// 扩张域 L 的 [`FieldExtension`]（若 L 为 registered 扩张）。
    pub fn extension_by_field(&self, field: FieldId) -> Option<&FieldExtension> {
        self.field_to_extension.get(&field).and_then(|id| self.extensions.get(id))
    }

    /// 自基域到 L 的域塔（升序，如 `[𝔽_p, …, L]`）。
    pub fn extension_tower(&self, extension: ExtensionId) -> Option<Vec<FieldId>> {
        let record = self.extensions.get(&extension)?.clone();
        Some(extension_tower_fields(&record, |field| self.extension_by_field(field).cloned()))
    }

    /// 注册扩张 Frobenius 自同构 σ^k（幂等）。
    pub fn register_frobenius_automorphism(&mut self, extension: ExtensionId, frobenius_power: u32) -> Result<AutomorphismId> {
        let field = self.extensions.get(&extension).ok_or_else(|| unknown_extension(extension))?.field;
        let presentation = self.presentation_id(field)?;
        Ok(self.map_table.register_frobenius_automorphism(extension, field, presentation, frobenius_power))
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
        let extension_id = ExtensionId(self.next_extension_id);
        self.next_extension_id = self.next_extension_id.wrapping_add(1);
        let field = FieldId(self.next_field_id);
        self.next_field_id = self.next_field_id.wrapping_add(1);
        let presentation_id = FieldPresentationId(self.next_presentation_id);
        self.next_presentation_id = self.next_presentation_id.wrapping_add(1);
        let kind = FieldPresentationKind::FiniteFieldPolynomialBasis { field, degree };
        let presentation = FieldPresentation { id: presentation_id, field, kind };
        self.by_key.insert(key, field);
        self.field_to_presentation.insert(field, presentation_id);
        self.presentations.insert(presentation_id, presentation);
        self.poly_extensions
            .insert(field, FiniteFieldPolySpec { extension: extension_id, base, characteristic, degree, modulus });
        let base_pres = self.presentation_id(base)?;
        let embedding = self.map_table.register_prime_subfield_embedding(base, field, base_pres, presentation_id);
        let ext = FieldExtension::finite_field_polynomial(extension_id, base, field, degree, embedding);
        self.extensions.insert(extension_id, ext);
        self.field_to_extension.insert(field, extension_id);
        Ok(field)
    }

    /// 数域规格（若已注册）。
    pub fn number_field_spec(&self, field: FieldId) -> Option<&NumberFieldSpec> {
        self.number_fields.get(&field)
    }

    /// 注册绝对数域 `ℚ(α)=ℚ[x]/(m)`。
    pub fn number_field_from_minimal_polynomial(&mut self, minimal_polynomial: Vec<Rational>) -> Result<FieldId> {
        let monic = make_monic(minimal_polynomial)?;
        let degree = validate_rational_modulus(&monic)?;
        if !is_irreducible_over_rationals(&monic)? {
            return Err(Diagnostic::new(DiagnosticCode::FieldModulusReducible)
                .detail("domain", "field")
                .detail("operation", "number_field_modulus"));
        }
        let key = FieldInternKey::NumberField { absolute_modulus: rational_key(&monic) };
        if let Some(&id) = self.by_key.get(&key) {
            return Ok(id);
        }
        let base = self.rationals();
        self.alloc_number_field(base, base, degree, degree, relative_modulus_from_rational(&monic, 1)?, monic, key)
    }

    /// 相对扩张：在数域（或 `ℚ`）上邻接有理系数首一不可约多项式的根。
    pub fn relative_number_field(&mut self, base: FieldId, relative_polynomial: Vec<Rational>) -> Result<FieldId> {
        let monic = make_monic(relative_polynomial)?;
        let relative_degree = validate_rational_modulus(&monic)?;
        if !is_irreducible_over_rationals(&monic)? {
            return Err(Diagnostic::new(DiagnosticCode::FieldModulusReducible)
                .detail("domain", "field")
                .detail("operation", "relative_number_field_modulus"));
        }
        let (absolute_base, base_degree, base_abs_mod) = match self.presentation(base).map(|p| &p.kind) {
            Some(FieldPresentationKind::Rationals) => {
                return self.number_field_from_minimal_polynomial(monic);
            }
            Some(FieldPresentationKind::NumberFieldPowerBasis { .. } | FieldPresentationKind::NumberFieldTower { .. }) => {
                let spec = self.number_fields.get(&base).ok_or_else(|| unknown_field(base))?;
                (spec.absolute_base, spec.absolute_degree, spec.absolute_modulus.clone())
            }
            _ => {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "field")
                    .detail("operation", "relative_base_not_number_field"));
            }
        };
        if relative_degree == 2 && base_degree == 2 && monic.len() == 3 && monic[1].is_zero() {
            if rational_is_square_in_quadratic(&monic[0].neg(), &base_abs_mod)? {
                return Err(Diagnostic::new(DiagnosticCode::FieldExtensionInvalid)
                    .detail("domain", "field")
                    .detail("operation", "relative_already_split"));
            }
        }
        let absolute_degree = absolute_degree_product(base_degree, relative_degree)?;
        let absolute_modulus = if base_degree == 1 {
            monic.clone()
        }
        else if base_degree == 2 && relative_degree == 2 && monic[1].is_zero() {
            biquadratic_absolute_modulus(&base_abs_mod, &monic[0].neg())?
        }
        else {
            let mut placeholder = vec![Rational::zero(); absolute_degree as usize + 1];
            placeholder[absolute_degree as usize] = Rational::one();
            placeholder
        };
        let key = FieldInternKey::NumberField { absolute_modulus: rational_key(&absolute_modulus) };
        if let Some(&id) = self.by_key.get(&key) {
            return Ok(id);
        }
        let relative_modulus = relative_modulus_from_rational(&monic, base_degree)?;
        self.alloc_number_field(base, absolute_base, relative_degree, absolute_degree, relative_modulus, absolute_modulus, key)
    }

    /// 元素在 `ℚ` 上的首一极小多项式。
    pub fn minimal_polynomial_over_rationals(&self, field: FieldId, coords: &[Rational]) -> Result<Vec<Rational>> {
        let spec = self.number_fields.get(&field).ok_or_else(|| unknown_field(field))?;
        let n = spec.absolute_degree as usize;
        if coords.len() != n {
            return Err(Diagnostic::new(DiagnosticCode::FieldElementInvalid)
                .detail("domain", "field")
                .detail("operation", "minpoly_coord_length"));
        }
        if coords.iter().all(|c| c.is_zero()) {
            return Ok(vec![Rational::zero(), Rational::one()]);
        }
        let mut powers = Vec::with_capacity(n + 1);
        let mut cur = {
            let mut one = vec![Rational::zero(); n];
            one[0] = Rational::one();
            one
        };
        for _ in 0..=n {
            powers.push(cur.clone());
            cur = self.mul_number_field_coords(field, &cur, coords)?;
        }
        crate::algebra::number_field::minimal_polynomial_from_powers(&powers)
    }

    fn mul_number_field_coords(&self, field: FieldId, a: &[Rational], b: &[Rational]) -> Result<Vec<Rational>> {
        let spec = self.number_fields.get(&field).ok_or_else(|| unknown_field(field))?;
        if spec.relative_degree == spec.absolute_degree {
            Ok(crate::algebra::number_field::mul_nf_coords(a, b, &spec.absolute_modulus))
        }
        else {
            let base_spec = self.number_fields.get(&spec.base).ok_or_else(|| unknown_field(spec.base))?;
            crate::algebra::number_field::mul_relative_nf_coords(
                a,
                b,
                &base_spec.absolute_modulus,
                &spec.relative_modulus,
                base_spec.absolute_degree,
                spec.relative_degree,
            )
        }
    }

    fn alloc_number_field(
        &mut self,
        base: FieldId,
        absolute_base: FieldId,
        relative_degree: u32,
        absolute_degree: u32,
        relative_modulus: Vec<Vec<Rational>>,
        absolute_modulus: Vec<Rational>,
        key: FieldInternKey,
    ) -> Result<FieldId> {
        let extension_id = ExtensionId(self.next_extension_id);
        self.next_extension_id = self.next_extension_id.wrapping_add(1);
        let field = FieldId(self.next_field_id);
        self.next_field_id = self.next_field_id.wrapping_add(1);
        let presentation_id = FieldPresentationId(self.next_presentation_id);
        self.next_presentation_id = self.next_presentation_id.wrapping_add(1);
        let kind = if matches!(self.presentation(base).map(|p| &p.kind), Some(FieldPresentationKind::Rationals)) {
            FieldPresentationKind::NumberFieldPowerBasis { extension: extension_id, degree: absolute_degree }
        }
        else {
            FieldPresentationKind::NumberFieldTower { base: self.presentation_id(base)?, extension: extension_id }
        };
        let presentation = FieldPresentation { id: presentation_id, field, kind };
        self.by_key.insert(key, field);
        self.field_to_presentation.insert(field, presentation_id);
        self.presentations.insert(presentation_id, presentation);
        self.number_fields.insert(
            field,
            NumberFieldSpec {
                extension: extension_id,
                base,
                absolute_base,
                relative_degree,
                absolute_degree,
                relative_modulus,
                absolute_modulus,
            },
        );
        let base_pres = self.presentation_id(base)?;
        let embedding = self.map_table.register_field_embedding(base, field, base_pres, presentation_id);
        let ext = FieldExtension::number_field(extension_id, base, field, relative_degree, embedding, true);
        self.extensions.insert(extension_id, ext);
        self.field_to_extension.insert(field, extension_id);
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

    /// 素域 𝔽_p 的约化模数（经 `FieldPresentation` 查找，系数内核真相源）。
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
    pub fn presentation_id(&self, field: FieldId) -> Result<FieldPresentationId> {
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
                    degree: PropertyState::Proven { value: *degree, witness: PropertyWitness::placeholder("polynomial_basis") },
                })
            }
            FieldPresentationKind::NumberFieldPowerBasis { degree, extension } => {
                let spec = self.number_fields.get(&field).ok_or_else(|| unknown_field(field))?;
                Ok(FieldDescriptor::Extension {
                    base: spec.base,
                    extension: *extension,
                    degree: PropertyState::Proven { value: *degree, witness: PropertyWitness::placeholder("number_field") },
                })
            }
            FieldPresentationKind::NumberFieldTower { extension, .. } => {
                let spec = self.number_fields.get(&field).ok_or_else(|| unknown_field(field))?;
                Ok(FieldDescriptor::Extension {
                    base: spec.base,
                    extension: *extension,
                    degree: PropertyState::Proven {
                        value: spec.relative_degree,
                        witness: PropertyWitness::placeholder("number_field_tower"),
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
        let presentation_id = FieldPresentationId(self.next_presentation_id);
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

fn unknown_extension(extension: ExtensionId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::FieldExtensionInvalid)
        .detail("domain", "field")
        .detail("operation", "unknown_extension")
        .detail("extension_id", extension.0.to_string())
}

fn rational_key(coeffs: &[Rational]) -> Vec<(Integer, Integer)> {
    coeffs.iter().map(|c| (c.numerator(), c.denominator())).collect()
}

fn is_square_rational(r: &Rational) -> bool {
    if r.is_negative() {
        return false;
    }
    let n = r.numerator().abs();
    let d = r.denominator();
    let sn = n.int_sqrt().expect("int_sqrt");
    let sd = d.int_sqrt().expect("int_sqrt");
    sn.mul(&sn) == n && sd.mul(&sd) == d
}

fn rational_is_square_in_quadratic(e: &Rational, base_abs_mod: &[Rational]) -> Result<bool> {
    if base_abs_mod.len() != 3 || base_abs_mod[2] != Rational::one() || !base_abs_mod[1].is_zero() {
        return Ok(false);
    }
    let d = base_abs_mod[0].neg();
    if is_square_rational(e) {
        return Ok(true);
    }
    if d.is_zero() {
        return Ok(false);
    }
    let ratio = e.try_div(&d).map_err(|_| {
        Diagnostic::new(DiagnosticCode::FieldExtensionInvalid)
            .detail("domain", "field")
            .detail("operation", "quadratic_square_test")
    })?;
    Ok(is_square_rational(&ratio))
}

fn biquadratic_absolute_modulus(base_abs_mod: &[Rational], d2: &Rational) -> Result<Vec<Rational>> {
    if base_abs_mod.len() != 3 {
        return Err(Diagnostic::new(DiagnosticCode::FieldExtensionInvalid)
            .detail("domain", "field")
            .detail("operation", "biquadratic_base"));
    }
    let d1 = base_abs_mod[0].neg();
    let two = Rational::from_integer(Integer::from_i64(2));
    let c2 = two.mul(&d1.add(d2)).neg();
    let diff = d1.sub(d2);
    let c0 = diff.mul(&diff);
    Ok(vec![c0, Rational::zero(), c2, Rational::zero(), Rational::one()])
}
