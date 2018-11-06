//! 系数环 intern 表（`CoefficientRingId` 与描述符 + 预计算模数）。

use std::collections::HashMap;

use athena_numeric::Modulus;
use athena_types::{CoefficientRingId, Diagnostic, DiagnosticCode};

use crate::algebra::{CoefficientParent, FieldTable};

use super::ring::{CoefficientDomain, RingCharacteristic, characteristic_of, validate_coefficient_domain_public};

/// 系数环完整描述符（`CoefficientRingId` 的内容）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoefficientRingDescriptor {
    /// Session 内句柄。
    pub id: CoefficientRingId,
    /// 系数域标签。
    pub domain: CoefficientDomain,
    /// 环特征。
    pub characteristic: RingCharacteristic,
    /// 系数父对象（Phase 1：有限域经 `FieldId`）。
    pub parent: Option<CoefficientParent>,
}

#[derive(Debug)]
pub(crate) struct CoeffRingEntry {
    descriptor: CoefficientRingDescriptor,
    prime_modulus: Option<Modulus>,
}

impl CoeffRingEntry {
    pub(crate) fn domain(&self) -> &CoefficientDomain {
        &self.descriptor.domain
    }

    pub(crate) fn prime_modulus(&self) -> Option<&Modulus> {
        self.prime_modulus.as_ref()
    }
}

/// Session 级系数环注册表（与 [`super::ring_table::RingTable`] 协同 intern）。
#[derive(Debug, Default)]
pub struct CoeffRingTable {
    next_id: u32,
    by_id: HashMap<CoefficientRingId, CoeffRingEntry>,
    by_key: HashMap<CoefficientDomain, CoefficientRingId>,
}

impl CoeffRingTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 内容寻址 intern。
    pub fn intern(&mut self, domain: CoefficientDomain, fields: &FieldTable) -> Result<CoefficientRingId, Diagnostic> {
        if let Some(&id) = self.by_key.get(&domain) {
            return Ok(id);
        }
        let domain = validate_coefficient_domain_public(domain)?;
        if matches!(domain, CoefficientDomain::FiniteField { .. }) {
            let CoefficientDomain::FiniteField { field, characteristic } = &domain
            else {
                unreachable!()
            };
            fields.validate_finite_field(*field, characteristic)?;
        }
        let characteristic = characteristic_of(&domain)?;
        let prime_modulus = prime_modulus_for(&domain, fields)?;
        let parent = coefficient_parent_of(&domain);
        let id = CoefficientRingId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let descriptor = CoefficientRingDescriptor { id, domain: domain.clone(), characteristic, parent };
        self.by_key.insert(domain, id);
        self.by_id.insert(id, CoeffRingEntry { descriptor, prime_modulus });
        Ok(id)
    }

    /// 按 id 查描述符。
    pub fn get(&self, id: CoefficientRingId) -> Option<&CoefficientRingDescriptor> {
        self.by_id.get(&id).map(|e| &e.descriptor)
    }

    /// 按 id 查 intern 条目（系数内核热路径）。
    pub(crate) fn entry(&self, id: CoefficientRingId) -> Result<&CoeffRingEntry, Diagnostic> {
        self.by_id.get(&id).ok_or_else(|| unknown_coeff_ring(id))
    }

    /// 系数父对象视图。
    pub fn coefficient_parent(&self, id: CoefficientRingId) -> Option<CoefficientParent> {
        self.get(id)?.parent
    }

    /// 已注册系数环数量。
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

fn coefficient_parent_of(domain: &CoefficientDomain) -> Option<CoefficientParent> {
    match domain {
        CoefficientDomain::FiniteField { field, .. } => Some(CoefficientParent::Field(*field)),
        _ => None,
    }
}

fn prime_modulus_for(domain: &CoefficientDomain, fields: &FieldTable) -> Result<Option<Modulus>, Diagnostic> {
    match domain {
        CoefficientDomain::PrimeField { p } => Ok(Some(Modulus::new(p.clone())?)),
        CoefficientDomain::FiniteField { field, .. } => {
            let p = fields.characteristic(*field).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "polynomial")
                    .detail("operation", "coeff_field_characteristic")
            })?;
            Ok(Some(Modulus::new(p)?))
        }
        _ => Ok(None),
    }
}

fn unknown_coeff_ring(id: CoefficientRingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_coefficient_ring")
        .detail("coefficient_ring_id", id.0.to_string())
}
