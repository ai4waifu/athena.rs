//! 级数 DomainObject 身份（session-local handle ≠ 裸 [`TermId`]）。

use crate::reasoning::mgraph::{ObjectRef, TheoryContextId};

use super::series::{Remainder, Series};

/// 会话局部的级数 `DomainObject` 句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SeriesRef(pub u64);

#[derive(Debug)]
struct SeriesObjectEntry {
    fingerprint: u64,
    series: Series,
}

/// Session 级数 DomainObject 仓。
#[derive(Debug, Default)]
pub struct SeriesObjectStore {
    entries: Vec<SeriesObjectEntry>,
}

impl SeriesObjectStore {
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

    /// Intern 级数；同 provisional 指纹复用 handle。
    pub fn intern(&mut self, series: Series) -> SeriesRef {
        let fingerprint = provisional_series_fingerprint(&series);
        if let Some((idx, _)) = self.entries.iter().enumerate().find(|(_, e)| e.fingerprint == fingerprint) {
            return SeriesRef(idx as u64);
        }
        let id = self.entries.len() as u64;
        self.entries.push(SeriesObjectEntry { fingerprint, series });
        SeriesRef(id)
    }

    /// 按 handle 取级数。
    pub fn get(&self, r: SeriesRef) -> Option<&Series> {
        self.entries.get(r.0 as usize).map(|e| &e.series)
    }

    /// Provisional 内容指纹（稳定 `SeriesFingerprint` 合同后续切片）。
    pub fn fingerprint(&self, r: SeriesRef) -> Option<u64> {
        self.entries.get(r.0 as usize).map(|e| e.fingerprint)
    }

    /// M-Graph 的 [`ObjectRef`]（`TheoryContextId::CALCULUS`）。
    pub fn object_ref(&self, r: SeriesRef) -> Option<ObjectRef> {
        self.fingerprint(r).map(|fp| ObjectRef::new(TheoryContextId::CALCULUS, fp))
    }
}

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn provisional_series_fingerprint(series: &Series) -> u64 {
    let mut body = Vec::with_capacity(32 + series.terms.len() * 12);
    body.extend_from_slice(b"ASR0");
    body.extend_from_slice(&series.variable.0.to_le_bytes());
    body.extend_from_slice(&series.center.0.to_le_bytes());
    body.extend_from_slice(&series.order.to_le_bytes());
    body.extend_from_slice(&(series.terms.len() as u32).to_le_bytes());
    for (coeff, power) in &series.terms {
        body.extend_from_slice(&coeff.0.to_le_bytes());
        body.extend_from_slice(&power.to_le_bytes());
    }
    match series.remainder {
        Remainder::ExactTruncation => body.push(0),
        Remainder::BigO(t) => {
            body.push(1);
            body.extend_from_slice(&t.0.to_le_bytes());
        }
        Remainder::LittleO(t) => {
            body.push(2);
            body.extend_from_slice(&t.0.to_le_bytes());
        }
        Remainder::Unknown => body.push(3),
    }
    fnv1a64(&body)
}
