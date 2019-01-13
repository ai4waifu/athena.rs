//! 系数环 intern 表（`CoefficientRingId` 与描述符 + 预计算模数）。

use std::collections::HashMap;

use athena_numeric::Modulus;
use athena_types::{CoefficientRingId, Diagnostic, DiagnosticCode, FieldId};

use crate::algebra::{CoefficientParent, FieldTable};

use super::{
    coeff_kernel::SpecializedCoeffKernel,
    ring::{CoefficientDomain, RingCharacteristic, characteristic_of, validate_coefficient_domain_public},
};

/// 系数环 intern 键（有限域仅 `FieldId`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CoeffRingInternKey {
    Domain(CoefficientDomain),
    Field(FieldId),
}

/// 系数环完整描述符（`CoefficientRingId` 的内容）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoefficientRingDescriptor {
    /// Session 内句柄。
    pub id: CoefficientRingId,
    /// 系数域标签（ℤ/ℚ/ℤ/nℤ；有限域仅 `FieldId`）。
    pub domain: CoefficientDomain,
    /// 环特征。
    pub characteristic: RingCharacteristic,
    /// 系数父对象。
    pub parent: CoefficientParent,
}

#[derive(Debug)]
pub(crate) struct CoeffRingEntry {
    descriptor: CoefficientRingDescriptor,
    /// 预计算模数（𝔽_p）；供未来 ℤ/nℤ kernel 复用。
    #[allow(dead_code)]
    prime_modulus: Option<Modulus>,
    /// 专用精确内核；ℤ/nℤ 等精确但非专用域为 `None`。
    kernel: Option<SpecializedCoeffKernel>,
}

impl CoeffRingEntry {
    pub(crate) fn kernel(&self) -> Option<&SpecializedCoeffKernel> {
        self.kernel.as_ref()
    }
}

/// Session 级系数环注册表（与 [`super::ring_table::RingTable`] 协同 intern）。
#[derive(Debug, Default)]
pub struct CoeffRingTable {
    next_id: u32,
    by_id: HashMap<CoefficientRingId, CoeffRingEntry>,
    by_key: HashMap<CoeffRingInternKey, CoefficientRingId>,
}

impl CoeffRingTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 内容寻址 intern。
    pub fn intern(&mut self, domain: CoefficientDomain, fields: &FieldTable) -> Result<CoefficientRingId, Diagnostic> {
        let key = intern_key(&domain);
        if let Some(&id) = self.by_key.get(&key) {
            return Ok(id);
        }
        let domain = validate_coefficient_domain_public(domain, fields)?;
        let characteristic = characteristic_of(&domain, fields)?;
        let prime_modulus = prime_modulus_for(&domain, fields)?;
        let kernel = if SpecializedCoeffKernel::supports(&domain) {
            Some(SpecializedCoeffKernel::build(&domain, prime_modulus.as_ref())?)
        }
        else {
            None
        };
        let id = CoefficientRingId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let parent = coefficient_parent_for(id, &domain);
        let descriptor = CoefficientRingDescriptor { id, domain: domain.clone(), characteristic, parent };
        self.by_key.insert(intern_key(&domain), id);
        self.by_id.insert(id, CoeffRingEntry { descriptor, prime_modulus, kernel });
        Ok(id)
    }

    /// 按 id 查描述符。
    pub fn get(&self, id: CoefficientRingId) -> Option<&CoefficientRingDescriptor> {
        self.by_id.get(&id).map(|e| &e.descriptor)
    }

    /// 按 id 查专用内核（算法热路径；非专用域报错）。
    pub(crate) fn kernel(&self, id: CoefficientRingId) -> Result<&SpecializedCoeffKernel, Diagnostic> {
        self.entry(id)?.kernel().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "polynomial")
                .detail("operation", "coeff_domain_unsupported")
                .detail("coefficient_ring_id", id.0.to_string())
        })
    }

    /// 按 id 查 intern 条目（系数内核热路径）。
    pub(crate) fn entry(&self, id: CoefficientRingId) -> Result<&CoeffRingEntry, Diagnostic> {
        self.by_id.get(&id).ok_or_else(|| unknown_coeff_ring(id))
    }

    /// 系数父对象视图。
    pub fn coefficient_parent(&self, id: CoefficientRingId) -> CoefficientParent {
        self.get(id).expect("valid coefficient ring").parent
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

fn intern_key(domain: &CoefficientDomain) -> CoeffRingInternKey {
    match domain {
        CoefficientDomain::FiniteField { field } => CoeffRingInternKey::Field(*field),
        other => CoeffRingInternKey::Domain(other.clone()),
    }
}

fn coefficient_parent_for(id: CoefficientRingId, domain: &CoefficientDomain) -> CoefficientParent {
    match domain {
        CoefficientDomain::FiniteField { field } => CoefficientParent::Field(*field),
        _ => CoefficientParent::Ring(id),
    }
}

fn prime_modulus_for(domain: &CoefficientDomain, fields: &FieldTable) -> Result<Option<Modulus>, Diagnostic> {
    match domain {
        CoefficientDomain::FiniteField { field } => Ok(Some(fields.prime_modulus(*field)?)),
        _ => Ok(None),
    }
}

fn unknown_coeff_ring(id: CoefficientRingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_coefficient_ring")
        .detail("coefficient_ring_id", id.0.to_string())
}
