//! 域 / 群稳定指纹（FNV-1a 64 · 不含 Session 句柄）。

use athena_numeric::{Integer, Number, serialization::NumericValueWire};
use athena_types::NumericKind;

use super::presentation::{FieldPresentationKind, GroupPresentationKind};

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

const FIELD_WIRE_MAGIC: &[u8; 4] = b"AFF1";
const GROUP_WIRE_MAGIC: &[u8; 4] = b"AGF1";

/// 域数学身份的稳定摘要（内容寻址；不含 [`athena_types::FieldId`]）。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldFingerprint(pub u64);

/// 群数学身份的稳定摘要（内容寻址；不含 [`athena_types::GroupId`]）。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupFingerprint(pub u64);

/// 域 presentation 内容指纹别名。
pub type FieldPresentationFingerprint = FieldFingerprint;

/// 群 presentation 内容指纹别名。
pub type GroupPresentationFingerprint = GroupFingerprint;

impl FieldFingerprint {
    /// ℚ。
    pub fn rationals() -> Self {
        Self::from_tag(1)
    }

    /// 素域 𝔽_p。
    pub fn prime_field(characteristic: &Integer) -> Self {
        let mut out = Vec::new();
        out.extend_from_slice(FIELD_WIRE_MAGIC);
        out.push(2);
        append_integer(&mut out, characteristic);
        Self(fnv1a64(&out))
    }

    /// 𝔽_{p^n} 多项式基：特征 + 不可约模多项式系数。
    pub fn finite_field_polynomial_basis(characteristic: &Integer, modulus: &[Integer]) -> Self {
        let mut out = Vec::new();
        out.extend_from_slice(FIELD_WIRE_MAGIC);
        out.push(3);
        append_integer(&mut out, characteristic);
        out.extend_from_slice(&(modulus.len() as u32).to_le_bytes());
        for c in modulus {
            append_integer(&mut out, c);
        }
        Self(fnv1a64(&out))
    }

    /// 数域：绝对极小多项式有理系数（分子/分母对）。
    pub fn number_field(absolute_modulus: &[(Integer, Integer)]) -> Self {
        let mut out = Vec::new();
        out.extend_from_slice(FIELD_WIRE_MAGIC);
        out.push(4);
        out.extend_from_slice(&(absolute_modulus.len() as u32).to_le_bytes());
        for (n, d) in absolute_modulus {
            append_integer(&mut out, n);
            append_integer(&mut out, d);
        }
        Self(fnv1a64(&out))
    }

    /// 仅由粗粒度 presentation kind 标签（无内容载荷时的占位）。
    pub fn from_presentation_kind_tag(kind: &FieldPresentationKind) -> Self {
        let tag = match kind {
            FieldPresentationKind::Rationals => 1u8,
            FieldPresentationKind::PrimeField { .. } => 2,
            FieldPresentationKind::FiniteFieldPolynomialBasis { .. } => 3,
            FieldPresentationKind::NumberFieldPowerBasis { .. } => 4,
            FieldPresentationKind::NumberFieldTower { .. } => 5,
            FieldPresentationKind::RationalFunctionField { .. } => 6,
            FieldPresentationKind::QuotientField { .. } => 7,
        };
        Self::from_tag(tag)
    }

    fn from_tag(tag: u8) -> Self {
        let mut out = Vec::new();
        out.extend_from_slice(FIELD_WIRE_MAGIC);
        out.push(tag);
        Self(fnv1a64(&out))
    }
}

impl GroupFingerprint {
    /// 置换群：degree + 生成元像（内容寻址）。
    pub fn from_permutation_generators(degree: u32, generators: &[Vec<u32>]) -> Self {
        let mut out = Vec::new();
        out.extend_from_slice(GROUP_WIRE_MAGIC);
        out.push(1);
        out.extend_from_slice(&degree.to_le_bytes());
        out.extend_from_slice(&(generators.len() as u32).to_le_bytes());
        for g in generators {
            out.extend_from_slice(&(g.len() as u32).to_le_bytes());
            for image in g {
                out.extend_from_slice(&image.to_le_bytes());
            }
        }
        Self(fnv1a64(&out))
    }

    /// 粗粒度 presentation kind（无生成元载荷）。
    pub fn from_presentation_kind(kind: &GroupPresentationKind) -> Self {
        let mut out = Vec::new();
        out.extend_from_slice(GROUP_WIRE_MAGIC);
        match kind {
            GroupPresentationKind::Permutation { degree } => {
                out.push(1);
                out.extend_from_slice(&degree.to_le_bytes());
            }
            GroupPresentationKind::ExplicitTable { order } => {
                out.push(2);
                append_integer(&mut out, order);
            }
            GroupPresentationKind::CyclicPresentation { order } => {
                out.push(3);
                append_integer(&mut out, order);
            }
            GroupPresentationKind::Pc => out.push(4),
            GroupPresentationKind::Matrix => out.push(5),
            GroupPresentationKind::FinitelyPresented => out.push(6),
            GroupPresentationKind::BlackBox => out.push(7),
        }
        Self(fnv1a64(&out))
    }
}

fn append_integer(out: &mut Vec<u8>, n: &Integer) {
    if let Ok(wire) = NumericValueWire::encode(&Number::integer(n.clone())) {
        out.push(match wire.kind {
            NumericKind::Integer => 0,
            NumericKind::Rational => 1,
            NumericKind::Real => 2,
            NumericKind::Complex => 3,
            NumericKind::Interval => 4,
            NumericKind::Algebraic => 5,
            NumericKind::FiniteField => 6,
            NumericKind::Modular => 7,
            NumericKind::PAdic => 8,
        });
        out.push(wire.sign);
        out.extend_from_slice(&(wire.payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&wire.payload);
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
