//! ℚ / 𝔽_p / 𝔽_{p^n} 元素 canonical 化与显式 embedding。

use athena_numeric::{Integer, Rational};
use athena_types::{AlgebraMapId, AutomorphismId, Diagnostic, DiagnosticCode, FieldId, Result};

use crate::algebra::{AlgebraParentId, FieldTable, MapTable, add_coords, canonical_coords, inv_coords, mul_coords};

use super::types::{FieldElement, FieldElementRepr};

/// 在 ℚ 中构造 canonical 元素（约分、正分母）。
pub fn canonical_rational(table: &FieldTable, field: FieldId, numer: Integer, denom: Integer) -> Result<FieldElement> {
    table.descriptor(field)?;
    let value = Rational::try_new(numer, denom).map_err(|_| field_element_invalid("rational_new"))?;
    let presentation = table.presentation_id(field)?;
    if !matches!(table.descriptor(field)?, super::types::FieldDescriptor::Rationals) {
        return Err(field_element_invalid("expected_rationals_field"));
    }
    Ok(FieldElement { field, presentation, repr: FieldElementRepr::Rational { value } })
}

/// 在 𝔽_{p^n} 多项式基中构造 canonical 元素。
pub fn canonical_extension_element(table: &FieldTable, field: FieldId, coords: Vec<Integer>) -> Result<FieldElement> {
    let spec = table.finite_field_poly_spec(field).ok_or_else(|| field_element_invalid("expected_polynomial_basis"))?;
    let p = table.prime_modulus(field)?;
    let coords = canonical_coords(coords, spec.degree, &p)?;
    let presentation = table.presentation_id(field)?;
    Ok(FieldElement { field, presentation, repr: FieldElementRepr::ExtensionCoords { coords } })
}

/// 在 𝔽_p 中构造 canonical 剩余类（∈ [0, p)）。
pub fn canonical_prime_residue(table: &FieldTable, field: FieldId, value: Integer) -> Result<FieldElement> {
    if table.finite_field_poly_spec(field).is_some() {
        return canonical_extension_element(table, field, vec![value]);
    }
    table.validate_finite_field(field)?;
    let modulus = table.prime_modulus(field)?;
    let residue = modulus.reduce(&value);
    let presentation = table.presentation_id(field)?;
    Ok(FieldElement { field, presentation, repr: FieldElementRepr::PrimeFieldResidue { value: residue } })
}

/// 经已注册素子域嵌入将 𝔽_p 元素映到 𝔽_{p^n}。
pub fn apply_prime_subfield_embedding(
    table: &FieldTable,
    maps: &MapTable,
    map_id: AlgebraMapId,
    element: &FieldElement,
) -> Result<FieldElement> {
    if !maps.is_prime_subfield_embedding(map_id) {
        return Err(field_element_invalid("not_prime_subfield_embedding"));
    }
    let map = maps.get(map_id).ok_or_else(|| field_element_invalid("unknown_map"))?;
    let (source, target) = field_endpoints(map)?;
    if element.field != source {
        return Err(field_mismatch());
    }
    match &element.repr {
        FieldElementRepr::PrimeFieldResidue { value } => {
            let spec = table.finite_field_poly_spec(target).ok_or_else(|| field_element_invalid("expected_extension"))?;
            let mut coords = vec![Integer::zero(); spec.degree as usize];
            coords[0] = value.clone();
            canonical_extension_element(table, target, coords)
        }
        _ => Err(field_element_invalid("embedding_source_not_prime_residue")),
    }
}

/// 对 𝔽_{p^n} 元素应用已注册 Frobenius 自同构。
pub fn apply_field_automorphism(
    table: &FieldTable,
    maps: &MapTable,
    aut: AutomorphismId,
    element: &FieldElement,
) -> Result<FieldElement> {
    let power = maps
        .automorphism_frobenius_power(aut)
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::AutomorphismInvalid).detail("domain", "field"))?;
    let extension = table.extension_by_field(element.field).ok_or_else(|| field_element_invalid("not_extension_field"))?;
    match &element.repr {
        FieldElementRepr::ExtensionCoords { coords } => {
            let next = crate::algebra::apply_frobenius_coords(table, extension.id, coords, power)?;
            canonical_extension_element(table, element.field, next)
        }
        _ => Err(field_element_invalid("automorphism_unsupported_repr")),
    }
}

/// 经已注册 canonical embedding 将 ℚ 元素映到 𝔽_p。
pub fn apply_field_embedding(
    table: &FieldTable,
    maps: &MapTable,
    map_id: AlgebraMapId,
    element: &FieldElement,
) -> Result<FieldElement> {
    let map = maps.get(map_id).ok_or_else(|| field_element_invalid("unknown_map"))?;
    let (source, target) = field_endpoints(map)?;
    if element.field != source {
        return Err(field_mismatch());
    }
    if !matches!(map.kind, crate::algebra::AlgebraMapKind::FieldEmbedding) {
        return Err(field_element_invalid("not_field_embedding"));
    }
    match &element.repr {
        FieldElementRepr::Rational { value } => {
            let modulus = table.prime_modulus(target)?;
            let numer = value.numerator();
            let denom = value.denominator();
            let inv = crate::number_theory::mod_inverse(&denom, &modulus)?;
            let residue = modulus.reduce(&numer.mul(inv.residue()));
            canonical_prime_residue(table, target, residue)
        }
        _ => Err(field_element_invalid("embedding_source_not_rational")),
    }
}

/// 同域元素加法（ℚ 或 𝔽_p）。
pub fn add_field_elements(table: &FieldTable, lhs: &FieldElement, rhs: &FieldElement) -> Result<FieldElement> {
    ensure_same_field(lhs, rhs)?;
    match (&lhs.repr, &rhs.repr) {
        (FieldElementRepr::Rational { value: a }, FieldElementRepr::Rational { value: b }) => {
            let sum = a.add(b);
            canonical_rational(table, lhs.field, sum.numerator(), sum.denominator())
        }
        (FieldElementRepr::PrimeFieldResidue { value: a }, FieldElementRepr::PrimeFieldResidue { value: b }) => {
            canonical_prime_residue(table, lhs.field, a.add(b))
        }
        (FieldElementRepr::ExtensionCoords { coords: a }, FieldElementRepr::ExtensionCoords { coords: b }) => {
            let p = table.prime_modulus(lhs.field)?;
            let sum = add_coords(a, b, &p);
            canonical_extension_element(table, lhs.field, sum)
        }
        _ => Err(field_element_invalid("add_repr_mismatch")),
    }
}

/// 同域元素乘法（ℚ 或 𝔽_p）。
pub fn mul_field_elements(table: &FieldTable, lhs: &FieldElement, rhs: &FieldElement) -> Result<FieldElement> {
    ensure_same_field(lhs, rhs)?;
    match (&lhs.repr, &rhs.repr) {
        (FieldElementRepr::Rational { value: a }, FieldElementRepr::Rational { value: b }) => {
            let product = a.mul(b);
            canonical_rational(table, lhs.field, product.numerator(), product.denominator())
        }
        (FieldElementRepr::PrimeFieldResidue { value: a }, FieldElementRepr::PrimeFieldResidue { value: b }) => {
            canonical_prime_residue(table, lhs.field, a.mul(b))
        }
        (FieldElementRepr::ExtensionCoords { coords: a }, FieldElementRepr::ExtensionCoords { coords: b }) => {
            let spec = table.finite_field_poly_spec(lhs.field).ok_or_else(|| field_element_invalid("extension_mul"))?;
            let p = table.prime_modulus(lhs.field)?;
            let prod = mul_coords(a, b, spec, &p);
            canonical_extension_element(table, lhs.field, prod)
        }
        _ => Err(field_element_invalid("mul_repr_mismatch")),
    }
}

/// 同域乘法逆元（ℚ 或 𝔽_p）。
pub fn inv_field_element(table: &FieldTable, element: &FieldElement) -> Result<FieldElement> {
    match &element.repr {
        FieldElementRepr::Rational { value } => {
            if value.is_zero() {
                return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "field"));
            }
            let inv = Rational::one().try_div(value)?;
            canonical_rational(table, element.field, inv.numerator(), inv.denominator())
        }
        FieldElementRepr::PrimeFieldResidue { value } => {
            let modulus = table.prime_modulus(element.field)?;
            let inv = crate::number_theory::mod_inverse(value, &modulus)?;
            canonical_prime_residue(table, element.field, inv.residue().clone())
        }
        FieldElementRepr::ExtensionCoords { coords } => {
            let spec = table.finite_field_poly_spec(element.field).ok_or_else(|| field_element_invalid("extension_inv"))?;
            let p = table.prime_modulus(element.field)?;
            let inv = inv_coords(coords, spec, &p)?;
            canonical_extension_element(table, element.field, inv)
        }
        _ => Err(field_element_invalid("inv_unsupported_repr")),
    }
}

fn ensure_same_field(lhs: &FieldElement, rhs: &FieldElement) -> Result<()> {
    if lhs.field != rhs.field || lhs.presentation != rhs.presentation {
        return Err(field_mismatch());
    }
    Ok(())
}

fn field_endpoints(map: &crate::algebra::AlgebraMap) -> Result<(FieldId, FieldId)> {
    match (map.source, map.target) {
        (AlgebraParentId::Field(s), AlgebraParentId::Field(t)) => Ok((s, t)),
        _ => Err(field_element_invalid("map_not_between_fields")),
    }
}

fn field_mismatch() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::FieldMismatch).detail("domain", "field")
}

fn field_element_invalid(operation: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::FieldElementInvalid).detail("domain", "field").detail("operation", operation)
}
