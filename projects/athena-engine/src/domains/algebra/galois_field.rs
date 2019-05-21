//! 有限域扩张上的 Galois 性质与伽罗瓦群（`𝔽_{pⁿ}/𝔽_p`）。

use athena_numeric::Integer;
use athena_types::{Diagnostic, DiagnosticCode, ExtensionId, Result};

use crate::domains::{
    algebra::{FieldTable, GroupTable, PropertyState, PropertyWitness, frobenius_power_coords},
    galois::{FieldAutomorphism, GaloisComputation, GaloisGroup},
    group::Permutation,
};

/// 扩张是否可分（`𝔽_{pⁿ}/𝔽_p` 恒为真）。
pub fn is_extension_separable(table: &FieldTable, extension: ExtensionId) -> Result<bool> {
    let record = table.extension_record(extension).ok_or_else(|| unknown_extension(extension))?;
    match &record.separable {
        PropertyState::Proven { value, .. } => Ok(*value),
        _ => Err(unsupported_extension(extension)),
    }
}

/// 扩张是否正规（`𝔽_{pⁿ}/𝔽_p` 恒为真）。
pub fn is_extension_normal(table: &FieldTable, extension: ExtensionId) -> Result<bool> {
    let record = table.extension_record(extension).ok_or_else(|| unknown_extension(extension))?;
    match &record.normal {
        PropertyState::Proven { value, .. } => Ok(*value),
        _ => Err(unsupported_extension(extension)),
    }
}

/// 扩张是否 Galois（正规且可分）。
pub fn is_galois_extension(table: &FieldTable, extension: ExtensionId) -> Result<bool> {
    Ok(is_extension_separable(table, extension)? && is_extension_normal(table, extension)?)
}

/// 构造 `𝔽_{pⁿ}/𝔽_p` 的完整伽罗瓦群（循环群 `Cₙ`）。
pub fn galois_group_of_extension(
    table: &mut FieldTable,
    groups: &mut GroupTable,
    extension: ExtensionId,
) -> Result<GaloisGroup> {
    let (base, field, degree) = {
        let record = table.extension_record(extension).ok_or_else(|| unknown_extension(extension))?;
        (record.base, record.field, record.proven_degree().ok_or_else(|| unsupported_extension(extension))?)
    };
    if table.finite_field_poly_spec(field).is_none() {
        return Err(unsupported_extension(extension));
    }
    for k in 0..degree {
        table.register_frobenius_automorphism(extension, k)?;
    }
    let group = groups.permutation_group(degree, &[cyclic_galois_generator(degree)])?;
    Ok(GaloisGroup { base_field: base, extension: Some(extension), computation: GaloisComputation::Complete { group } })
}

/// 返回 Frobenius 自同构 σ^k（必要时注册）。
pub fn field_automorphism(table: &mut FieldTable, extension: ExtensionId, frobenius_power: u32) -> Result<FieldAutomorphism> {
    let aut_id = table.register_frobenius_automorphism(extension, frobenius_power)?;
    let map_id = table
        .map_table()
        .automorphism_map(aut_id)
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::AutomorphismInvalid).detail("domain", "galois"))?;
    let degree = table.extension_record(extension).and_then(|r| r.proven_degree()).unwrap_or(1);
    let inverse = if frobenius_power == 0 {
        None
    }
    else {
        Some(table.register_frobenius_automorphism(extension, degree - frobenius_power)?)
    };
    Ok(FieldAutomorphism {
        id: aut_id,
        extension,
        embedding: map_id,
        fixes_base: PropertyState::Proven { value: true, witness: PropertyWitness::placeholder("frobenius_fixes_fp") },
        inverse,
    })
}

/// 对扩张坐标应用 σ^k。
pub fn apply_frobenius_coords(
    table: &FieldTable,
    extension: ExtensionId,
    coords: &[Integer],
    power: u32,
) -> Result<Vec<Integer>> {
    let record = table.extension_record(extension).ok_or_else(|| unknown_extension(extension))?;
    let spec = table.finite_field_poly_spec(record.field).ok_or_else(|| unsupported_extension(extension))?;
    let p = table.prime_modulus(record.field)?;
    Ok(frobenius_power_coords(coords, power, spec, &p))
}

fn cyclic_galois_generator(degree: u32) -> Permutation {
    let mut images: Vec<u32> = (0..degree).collect();
    if degree > 1 {
        for i in 0..degree as usize {
            images[i] = ((i as u32 + 1) % degree) as u32;
        }
    }
    Permutation { images }
}

fn unknown_extension(extension: ExtensionId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::FieldExtensionInvalid)
        .detail("domain", "galois")
        .detail("operation", "unknown_extension")
        .detail("extension_id", extension.0.to_string())
}

fn unsupported_extension(extension: ExtensionId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "galois")
        .detail("operation", "extension_not_finite_field_polynomial")
        .detail("extension_id", extension.0.to_string())
}
