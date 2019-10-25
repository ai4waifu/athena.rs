//! Living `28` 多项式 DomainObject 身份（session-local handle ≠ 裸 [`TermId`]）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::reasoning::mgraph::{ObjectRef, TheoryContextId};

use super::{
    fingerprint::{PolynomialFingerprint, fnv1a64, polynomial_fingerprint},
    object::Polynomial,
    request::PolynomialRequest,
    ring_table::RingTable,
};

/// Session-local polynomial DomainObject handle（Living `28`）。
///
/// 稳定数学身份用 [`PolynomialFingerprint`] / [`ObjectRef`]；本类型只在当前 Session 内寻址 payload。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PolynomialRef(pub u64);

#[derive(Debug)]
struct PolynomialObjectEntry {
    fingerprint: PolynomialFingerprint,
    poly: Polynomial,
}

/// Session 多项式 DomainObject 仓。
#[derive(Debug, Default)]
pub struct PolynomialObjectStore {
    entries: Vec<PolynomialObjectEntry>,
}

impl PolynomialObjectStore {
    /// 空仓。
    pub fn new() -> Self {
        Self::default()
    }

    /// 条目数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Intern 多项式；同指纹复用已有 [`PolynomialRef`]。
    pub fn intern(&mut self, poly: Polynomial, rings: &RingTable) -> PolynomialRef {
        let fingerprint = fingerprint_or_provisional(&poly, rings);
        if let Some((idx, _)) = self.entries.iter().enumerate().find(|(_, e)| e.fingerprint == fingerprint) {
            return PolynomialRef(idx as u64);
        }
        let id = self.entries.len() as u64;
        self.entries.push(PolynomialObjectEntry { fingerprint, poly });
        PolynomialRef(id)
    }

    /// 按 handle 取多项式。
    pub fn get(&self, r: PolynomialRef) -> Option<&Polynomial> {
        self.entries.get(r.0 as usize).map(|e| &e.poly)
    }

    /// 解析为 owning 副本（算法入口）。
    pub fn resolve_owning(&self, r: PolynomialRef) -> Result<Polynomial> {
        self.get(r).map(Polynomial::owning_copy).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "polynomial")
                .detail("reason", "missing_polynomial_ref")
                .arg("ref", r.0)
        })
    }

    /// 稳定（或 provisional）指纹。
    pub fn fingerprint(&self, r: PolynomialRef) -> Option<PolynomialFingerprint> {
        self.entries.get(r.0 as usize).map(|e| e.fingerprint)
    }

    /// M-Graph [`ObjectRef`]（`TheoryContextId::POLYNOMIAL`）。
    pub fn object_ref(&self, r: PolynomialRef) -> Option<ObjectRef> {
        self.fingerprint(r).map(|fp| ObjectRef::new(TheoryContextId::POLYNOMIAL, fp.0))
    }
}

fn fingerprint_or_provisional(poly: &Polynomial, rings: &RingTable) -> PolynomialFingerprint {
    match polynomial_fingerprint(poly, rings) {
        Ok(fp) => fp,
        Err(_) => {
            let mut body = Vec::with_capacity(16);
            body.extend_from_slice(b"APP0");
            body.extend_from_slice(&poly.ring().0.to_le_bytes());
            body.extend_from_slice(&(poly.terms().len() as u32).to_le_bytes());
            PolynomialFingerprint(fnv1a64(&body))
        }
    }
}

/// Collect DomainObject handles already present on a typed request.
pub fn refs_from_request(request: &PolynomialRequest) -> Vec<PolynomialRef> {
    match request {
        PolynomialRequest::Normalize { polynomial } | PolynomialRequest::Factor { polynomial, .. } => vec![*polynomial],
        PolynomialRequest::Add { lhs, rhs } | PolynomialRequest::Mul { lhs, rhs } | PolynomialRequest::Gcd { lhs, rhs } => {
            vec![*lhs, *rhs]
        }
        PolynomialRequest::Div { dividend, divisor, .. } => vec![*dividend, *divisor],
        PolynomialRequest::Groebner { generators, .. } | PolynomialRequest::Eliminate { generators, .. } => generators.clone(),
    }
}

/// Build M-Graph [`ObjectRef`] list for handles (skips missing slots).
pub fn object_refs_for(store: &PolynomialObjectStore, refs: &[PolynomialRef]) -> Vec<ObjectRef> {
    refs.iter().filter_map(|r| store.object_ref(*r)).collect()
}

/// Convenience: collect [`ObjectRef`]s for a request's handles.
pub fn intern_request_object_refs(
    request: &PolynomialRequest,
    _rings: &RingTable,
    store: &mut PolynomialObjectStore,
) -> Result<Vec<ObjectRef>> {
    let refs = refs_from_request(request);
    Ok(object_refs_for(store, &refs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use athena_types::RingId;

    #[test]
    fn intern_dedupes_by_fingerprint() {
        let rings = RingTable::default();
        let mut store = PolynomialObjectStore::new();
        let a = store.intern(Polynomial::zero(RingId(0)), &rings);
        let b = store.intern(Polynomial::zero(RingId(0)), &rings);
        assert_eq!(a, b);
        assert_eq!(store.len(), 1);
        assert!(store.object_ref(a).is_some());
    }
}
