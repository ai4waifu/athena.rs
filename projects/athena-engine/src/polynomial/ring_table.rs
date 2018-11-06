//! 多项式环 intern 表（Session 级内容寻址）。

use std::collections::HashMap;

use athena_numeric::Integer;
use athena_types::{CoefficientRingId, Diagnostic, RingId, SymbolId};

use crate::algebra::{CoefficientParent, FieldTable};

use super::{
    coeff_kernel::CoeffRing,
    coeff_ring_table::CoeffRingTable,
    fingerprint::{RingFingerprint, RingHandle},
    order::MonomialOrder,
    ring::{CoefficientDomain, RingDescriptor, validate_coefficient_domain_public, validate_prime_modulus},
};

/// 环 intern 键（系数环 id + 变量 + 序）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RingInternKey {
    coefficient_ring: CoefficientRingId,
    variables: Vec<SymbolId>,
    order: MonomialOrder,
}

/// Session 持有的多项式环注册表（内嵌 [`CoeffRingTable`] 与 [`FieldTable`]）。
#[derive(Debug, Default)]
pub struct RingTable {
    coeff_rings: CoeffRingTable,
    fields: FieldTable,
    next_id: u32,
    by_id: HashMap<RingHandle, RingDescriptor>,
    by_key: HashMap<RingInternKey, RingHandle>,
}

impl RingTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 域注册表（只读）。
    pub fn field_table(&self) -> &FieldTable {
        &self.fields
    }

    /// 域注册表（可变）。
    pub fn field_table_mut(&mut self) -> &mut FieldTable {
        &mut self.fields
    }

    /// 系数环 intern 表（只读）。
    pub fn coeff_rings(&self) -> &CoeffRingTable {
        &self.coeff_rings
    }

    /// 解析环上的专用系数内核。
    pub fn coeff_kernel(&self, ring: RingHandle) -> Result<CoeffRing<'_>, Diagnostic> {
        let desc = self.get(ring).ok_or_else(|| ring_unknown(ring))?;
        CoeffRing::for_descriptor(desc.coefficient_ring, &self.coeff_rings)
    }

    /// 系数父对象（Living `18` Phase 1）。
    pub fn coefficient_parent(&self, ring: RingHandle) -> Option<CoefficientParent> {
        let desc = self.get(ring)?;
        self.coeff_rings.coefficient_parent(desc.coefficient_ring)
    }

    /// 查环的稳定数学指纹。
    pub fn ring_fingerprint(&self, ring: RingHandle) -> Option<RingFingerprint> {
        self.get(ring).map(|d| d.ring_fingerprint)
    }

    /// 经 [`FieldTable::prime_field`] 注册 𝔽_p 后构造多项式环（推荐路径）。
    pub fn intern_over_prime_field(
        &mut self,
        p: Integer,
        variables: Vec<SymbolId>,
        order: MonomialOrder,
    ) -> Result<RingHandle, Diagnostic> {
        let field = self.fields.prime_field(p.clone())?;
        self.intern(CoefficientDomain::FiniteField { field, characteristic: p }, variables, order)
    }

    /// 内容寻址 intern；`PrimeField { p }` 规范化为 `FiniteField { field, characteristic: p }`。
    pub fn intern(
        &mut self,
        coefficients: CoefficientDomain,
        variables: Vec<SymbolId>,
        order: MonomialOrder,
    ) -> Result<RingHandle, Diagnostic> {
        let coefficients = normalize_coefficient_domain(coefficients, &mut self.fields)?;
        let (coefficients, variables, order, characteristic) =
            RingDescriptor::validate_content(coefficients, variables, order)?;
        let coefficient_ring = self.coeff_rings.intern(coefficients.clone(), &self.fields)?;
        let key = RingInternKey { coefficient_ring, variables: variables.clone(), order: order.clone() };
        if let Some(&id) = self.by_key.get(&key) {
            return Ok(id);
        }
        let ring_fingerprint = RingFingerprint::from_parts(&coefficients, &variables, &order);
        let id = RingId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let desc = RingDescriptor::with_id(
            id,
            coefficient_ring,
            coefficients,
            variables,
            order,
            characteristic,
            ring_fingerprint,
        );
        self.by_key.insert(key, id);
        self.by_id.insert(id, desc);
        Ok(id)
    }

    /// 按 id 查描述符。
    pub fn get(&self, id: RingHandle) -> Option<&RingDescriptor> {
        self.by_id.get(&id)
    }

    /// 已注册环数量。
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

fn normalize_coefficient_domain(
    coefficients: CoefficientDomain,
    fields: &mut FieldTable,
) -> Result<CoefficientDomain, Diagnostic> {
    match coefficients {
        CoefficientDomain::PrimeField { p } => {
            validate_prime_modulus(&p)?;
            let field = fields.prime_field(p.clone())?;
            Ok(CoefficientDomain::FiniteField { field, characteristic: p })
        }
        CoefficientDomain::FiniteField { field, characteristic } => {
            fields.validate_finite_field(field, &characteristic)?;
            Ok(CoefficientDomain::FiniteField { field, characteristic })
        }
        other => validate_coefficient_domain_public(other),
    }
}

fn ring_unknown(ring: RingHandle) -> Diagnostic {
    athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
