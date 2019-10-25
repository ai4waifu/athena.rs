//! Living `28` 矩阵 DomainObject 身份（session-local handle ≠ 裸 [`TermId`]）。
//!
//! Living `07` 的 `MatrixId` 语义在此以 [`MatrixRef`] 落地（与 `PolynomialRef` / `SeriesRef` 对齐）。

use crate::reasoning::mgraph::{ObjectRef, TheoryContextId};

use super::value::MatrixValue;

/// Session-local matrix DomainObject handle（Living `28`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MatrixRef(pub u64);

#[derive(Debug)]
struct MatrixObjectEntry {
    fingerprint: u64,
    matrix: MatrixValue,
}

/// Session 矩阵 DomainObject 仓。
#[derive(Debug, Default)]
pub struct MatrixObjectStore {
    entries: Vec<MatrixObjectEntry>,
}

impl MatrixObjectStore {
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

    /// Intern 矩阵；同 provisional 指纹复用 handle。
    pub fn intern(&mut self, matrix: MatrixValue) -> MatrixRef {
        let fingerprint = provisional_matrix_fingerprint(&matrix);
        if let Some((idx, _)) = self.entries.iter().enumerate().find(|(_, e)| e.fingerprint == fingerprint) {
            return MatrixRef(idx as u64);
        }
        let id = self.entries.len() as u64;
        self.entries.push(MatrixObjectEntry { fingerprint, matrix });
        MatrixRef(id)
    }

    /// 按 handle 取矩阵。
    pub fn get(&self, r: MatrixRef) -> Option<&MatrixValue> {
        self.entries.get(r.0 as usize).map(|e| &e.matrix)
    }

    /// 解析为 owning 副本。
    pub fn resolve_owning(&self, r: MatrixRef) -> Option<MatrixValue> {
        self.get(r).map(MatrixValue::owning_copy)
    }

    /// Provisional 内容指纹（稳定 `MatrixFingerprint` 合同后续切片）。
    pub fn fingerprint(&self, r: MatrixRef) -> Option<u64> {
        self.entries.get(r.0 as usize).map(|e| e.fingerprint)
    }

    /// M-Graph [`ObjectRef`]（`TheoryContextId::MATRIX`）。
    pub fn object_ref(&self, r: MatrixRef) -> Option<ObjectRef> {
        self.fingerprint(r).map(|fp| ObjectRef::new(TheoryContextId::MATRIX, fp))
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

fn provisional_matrix_fingerprint(matrix: &MatrixValue) -> u64 {
    use super::{
        parent::ElementParentKind,
        shape::StorageOrder,
        value::MatrixBuffer,
    };
    let mut body = Vec::with_capacity(64);
    body.extend_from_slice(b"AMX0");
    body.extend_from_slice(&matrix.shape().rows.to_le_bytes());
    body.extend_from_slice(&matrix.shape().cols.to_le_bytes());
    body.extend_from_slice(&(matrix.offset() as i64).to_le_bytes());
    body.push(match matrix.parent().element {
        ElementParentKind::Integers => 0,
        ElementParentKind::Rationals => 1,
        ElementParentKind::MachineReal => 2,
    });
    body.push(match matrix.layout().order {
        StorageOrder::RowMajor => 0,
        StorageOrder::ColumnMajor => 1,
    });
    match matrix.buffer() {
        MatrixBuffer::Integers(v) => {
            body.push(0);
            body.extend_from_slice(&(v.len() as u32).to_le_bytes());
            for n in v.iter() {
                body.extend_from_slice(n.to_decimal_string().as_bytes());
                body.push(0xff);
            }
        }
        MatrixBuffer::Rationals(v) => {
            body.push(1);
            body.extend_from_slice(&(v.len() as u32).to_le_bytes());
            for n in v.iter() {
                body.extend_from_slice(n.to_wire_string().as_bytes());
                body.push(0xff);
            }
        }
        MatrixBuffer::MachineF64(v) => {
            body.push(2);
            body.extend_from_slice(&(v.len() as u32).to_le_bytes());
            for x in v.iter() {
                body.extend_from_slice(&x.to_bits().to_le_bytes());
            }
        }
    }
    fnv1a64(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use athena_numeric::Integer;

    #[test]
    fn intern_dedupes_identical_matrices() {
        let mut store = MatrixObjectStore::new();
        let a = MatrixValue::from_integers_row_major(1, 2, vec![Integer::from_i64(1), Integer::from_i64(2)]).unwrap();
        let b = MatrixValue::from_integers_row_major(1, 2, vec![Integer::from_i64(1), Integer::from_i64(2)]).unwrap();
        let r0 = store.intern(a);
        let r1 = store.intern(b);
        assert_eq!(r0, r1);
        assert_eq!(store.len(), 1);
        assert!(store.object_ref(r0).is_some());
    }
}
