//! 多项式环 intern 表（Session 级内容寻址）。

use std::collections::HashMap;

use athena_numeric::Integer;
use athena_types::{CoefficientRingId, Diagnostic, FieldId, RingId, SymbolId};

use crate::algebra::{CoefficientParent, FieldTable};

use super::{
    coeff_kernel::CoeffRing,
    coeff_ring_table::CoeffRingTable,
    fingerprint::{RingFingerprint, RingHandle},
    monomial_layout::MonomialLayout,
    order::MonomialOrder,
    ring::{CoefficientDomain, RingDescriptor, validate_coefficient_domain_public},
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

    /// 系数域标签（经 [`RingDescriptor::coefficient_ring`] 解析）。
    pub fn coefficient_domain(&self, ring: RingHandle) -> Option<&CoefficientDomain> {
        let desc = self.get(ring)?;
        self.coefficient_domain_for_descriptor(desc)
    }

    pub(crate) fn coefficient_domain_for_descriptor(&self, desc: &RingDescriptor) -> Option<&CoefficientDomain> {
        self.coeff_rings.get(desc.coefficient_ring).map(|d| &d.domain)
    }

    /// 解析环上的专用系数内核。
    pub fn coeff_kernel(&self, ring: RingHandle) -> Result<CoeffRing<'_>, Diagnostic> {
        let desc = self.get(ring).ok_or_else(|| ring_unknown(ring))?;
        CoeffRing::for_descriptor(desc.coefficient_ring, &self.coeff_rings)
    }

    /// 系数父对象（`RingDescriptor.coefficients`）。
    pub fn coefficient_parent(&self, ring: RingHandle) -> Option<CoefficientParent> {
        self.get(ring).map(|d| d.coefficients)
    }

    /// 查环的稳定数学指纹。
    pub fn ring_fingerprint(&self, ring: RingHandle) -> Option<RingFingerprint> {
        self.get(ring).map(|d| d.ring_fingerprint)
    }

    /// 经已注册 [`FieldId`] 构造多项式环（特征与模数由 presentation 提供）。
    pub fn intern_over_field(
        &mut self,
        field: FieldId,
        variables: Vec<SymbolId>,
        order: MonomialOrder,
    ) -> Result<RingHandle, Diagnostic> {
        self.intern(CoefficientDomain::FiniteField { field }, variables, order)
    }

    /// 经 [`FieldTable::prime_field`] 注册 𝔽_p 后构造多项式环（素域推荐路径）。
    pub fn intern_over_prime_field(
        &mut self,
        p: Integer,
        variables: Vec<SymbolId>,
        order: MonomialOrder,
    ) -> Result<RingHandle, Diagnostic> {
        let field = self.fields.prime_field(p)?;
        self.intern_over_field(field, variables, order)
    }

    /// 内容寻址 intern。
    pub fn intern(
        &mut self,
        coefficients: CoefficientDomain,
        variables: Vec<SymbolId>,
        order: MonomialOrder,
    ) -> Result<RingHandle, Diagnostic> {
        let coefficients = normalize_coefficient_domain(coefficients, &mut self.fields)?;
        let (domain, variables, order, characteristic) =
            RingDescriptor::validate_content(coefficients, variables, order, &self.fields)?;
        let coefficient_ring = self.coeff_rings.intern(domain.clone(), &self.fields)?;
        let coefficients = self.coeff_rings.coefficient_parent(coefficient_ring);
        let key = RingInternKey { coefficient_ring, variables: variables.clone(), order: order.clone() };
        if let Some(&id) = self.by_key.get(&key) {
            return Ok(id);
        }
        let ring_fingerprint = RingFingerprint::from_parts(&domain, &variables, &order, &self.fields);
        let monomial_layout = MonomialLayout::compile(&order, variables.len())?;
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
            monomial_layout,
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
        CoefficientDomain::FiniteField { field } => {
            fields.validate_finite_field(field)?;
            Ok(CoefficientDomain::FiniteField { field })
        }
        other => validate_coefficient_domain_public(other, fields),
    }
}

fn ring_unknown(ring: RingHandle) -> Diagnostic {
    athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
