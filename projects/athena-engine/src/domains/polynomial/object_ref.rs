//! Living `28` 多项式 DomainObject 身份（session-local handle ≠ 裸 [`TermId`]）。

use athena_types::Result;

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

/// Intern every polynomial payload in a request. Returns DomainObject handles in request order.
pub fn intern_polynomial_request(request: &PolynomialRequest, rings: &RingTable, store: &mut PolynomialObjectStore) -> Vec<PolynomialRef> {
    let mut out = Vec::new();
    match request {
        PolynomialRequest::Normalize { polynomial }
        | PolynomialRequest::Factor { polynomial, .. } => {
            out.push(store.intern(polynomial.owning_copy(), rings));
        }
        PolynomialRequest::Add { lhs, rhs }
        | PolynomialRequest::Mul { lhs, rhs }
        | PolynomialRequest::Gcd { lhs, rhs } => {
            out.push(store.intern(lhs.owning_copy(), rings));
            out.push(store.intern(rhs.owning_copy(), rings));
        }
        PolynomialRequest::Div { dividend, divisor, .. } => {
            out.push(store.intern(dividend.owning_copy(), rings));
            out.push(store.intern(divisor.owning_copy(), rings));
        }
        PolynomialRequest::Groebner { generators, .. } | PolynomialRequest::Eliminate { generators, .. } => {
            for g in generators {
                out.push(store.intern(g.owning_copy(), rings));
            }
        }
    }
    out
}

/// Build M-Graph [`ObjectRef`] list for interned handles (skips missing slots).
pub fn object_refs_for(store: &PolynomialObjectStore, refs: &[PolynomialRef]) -> Vec<ObjectRef> {
    refs.iter().filter_map(|r| store.object_ref(*r)).collect()
}

/// Convenience: intern request bodies and collect [`ObjectRef`]s.
pub fn intern_request_object_refs(
    request: &PolynomialRequest,
    rings: &RingTable,
    store: &mut PolynomialObjectStore,
) -> Result<Vec<ObjectRef>> {
    let refs = intern_polynomial_request(request, rings, store);
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
