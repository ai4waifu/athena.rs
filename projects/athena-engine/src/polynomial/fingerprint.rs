//! 稳定环 / 多项式指纹（FNV-1a 64 · 规范二进制编码 · 不含 Session 句柄）。

use athena_numeric::{Integer, Number, serialization::NumericValueWire};
use athena_types::{Diagnostic, DiagnosticCode, NumericKind, Result, SymbolId};

use crate::{algebra::FieldTable, numeric_clone::clone_integer};

use super::{
    expr::Polynomial,
    order::MonomialOrder,
    ring::{CoefficientDomain, RingDescriptor},
    ring_table::RingTable,
};

/// 指纹算法标识（FNV-1a 64-bit，跨 Rust 版本稳定）。
pub const FINGERPRINT_ALGORITHM: &str = "fnv1a64-v1";

/// 环描述符 wire schema。
const RING_WIRE_MAGIC: &[u8; 4] = b"APR1";

/// 多项式 body wire schema。
const POLY_WIRE_MAGIC: &[u8; 4] = b"APP1";

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// Session 内多项式环快速索引（[`RingId`] 别名，非稳定数学身份）。
pub type RingHandle = athena_types::RingId;

/// 环数学身份的稳定摘要（由系数域 · 变量 · 单项式序编码；不含 [`RingHandle`]）。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RingFingerprint(pub u64);

/// Canonical 多项式内容的稳定摘要（[`RingFingerprint`] + 系数 wire + 指数布局）。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PolynomialFingerprint(pub u64);

impl RingFingerprint {
    /// 由环描述符计算（intern 时调用一次）。
    pub fn from_descriptor(desc: &RingDescriptor, domain: &CoefficientDomain, fields: &FieldTable) -> Self {
        Self::from_parts(domain, &desc.variables, &desc.order, fields)
    }

    /// 由环内容分量计算（intern 前无 [`RingHandle`] 时使用）。
    pub fn from_parts(
        coefficients: &CoefficientDomain,
        variables: &[SymbolId],
        order: &MonomialOrder,
        fields: &FieldTable,
    ) -> Self {
        let mut out = Vec::new();
        out.extend_from_slice(RING_WIRE_MAGIC);
        encode_coefficient_domain(coefficients, fields, &mut out);
        out.extend_from_slice(&(variables.len() as u32).to_le_bytes());
        for v in variables {
            out.extend_from_slice(&v.0.to_le_bytes());
        }
        encode_monomial_order(order, &mut out);
        Self(fnv1a64(&out))
    }
}

impl PolynomialFingerprint {
    /// 由 canonical 多项式计算。
    pub fn from_polynomial(poly: &Polynomial, rings: &RingTable) -> Result<Self> {
        let desc = rings.get(poly.ring()).ok_or_else(|| unknown_ring(poly.ring()))?;
        let ring_fp = desc.ring_fingerprint;
        let mut body = Vec::new();
        body.extend_from_slice(POLY_WIRE_MAGIC);
        body.extend_from_slice(&ring_fp.0.to_le_bytes());
        encode_polynomial_body(poly, desc.variable_count(), &mut body)?;
        Ok(Self(fnv1a64(&body)))
    }
}

/// 对 canonical 多项式求稳定指纹（M-Graph / 缓存 / witness 合同）。
pub fn polynomial_fingerprint(poly: &Polynomial, rings: &RingTable) -> Result<PolynomialFingerprint> {
    PolynomialFingerprint::from_polynomial(poly, rings)
}

/// 兼容旧 API：返回 [`PolynomialFingerprint`] 的 `u64` 载荷。
pub fn polynomial_fingerprint_u64(poly: &Polynomial, rings: &RingTable) -> Result<u64> {
    Ok(polynomial_fingerprint(poly, rings)?.0)
}

/// FNV-1a 64-bit（固定算法，非 `DefaultHasher`）。
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn encode_polynomial_body(poly: &Polynomial, variable_count: usize, out: &mut Vec<u8>) -> Result<()> {
    out.extend_from_slice(&(poly.terms().len() as u32).to_le_bytes());
    for term in poly.terms() {
        if term.exponents().len() != variable_count {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "fingerprint_exponent_length"));
        }
        append_number_wire(out, term.coefficient())?;
        out.extend_from_slice(&(term.exponents().len() as u32).to_le_bytes());
        for e in term.exponents() {
            out.extend_from_slice(&e.to_le_bytes());
        }
    }
    Ok(())
}

fn encode_coefficient_domain(domain: &CoefficientDomain, fields: &FieldTable, out: &mut Vec<u8>) {
    match domain {
        CoefficientDomain::Integer => {
            out.push(0);
        }
        CoefficientDomain::Rational => {
            out.push(1);
        }
        CoefficientDomain::ModularInteger { modulus } => {
            out.push(2);
            append_integer_wire_infallible(out, &modulus.value());
        }
        CoefficientDomain::FiniteField { field } => {
            out.push(3);
            let characteristic = fields.characteristic(*field).expect("validated field");
            append_integer_wire_infallible(out, &characteristic);
            out.extend_from_slice(&field.0.to_le_bytes());
        }
        CoefficientDomain::ApproximateReal => {
            out.push(255);
        }
    }
}

fn encode_monomial_order(order: &MonomialOrder, out: &mut Vec<u8>) {
    match order {
        MonomialOrder::Lex => out.push(0),
        MonomialOrder::GrLex => out.push(1),
        MonomialOrder::GrevLex => out.push(2),
        MonomialOrder::Weighted { weights } => {
            out.push(3);
            out.extend_from_slice(&(weights.len() as u32).to_le_bytes());
            for w in weights {
                out.extend_from_slice(&w.to_le_bytes());
            }
        }
        MonomialOrder::Block { blocks } => {
            out.push(4);
            out.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
            for b in blocks {
                encode_monomial_order(b, out);
            }
        }
        MonomialOrder::Elimination { eliminate, rest } => {
            out.push(5);
            out.extend_from_slice(&eliminate.to_le_bytes());
            encode_monomial_order(rest, out);
        }
    }
}

fn append_integer_wire_infallible(out: &mut Vec<u8>, n: &Integer) {
    if let Ok(wire) = NumericValueWire::encode(&Number::integer(clone_integer(n))) {
        out.push(numeric_kind_tag(wire.kind));
        out.push(wire.sign);
        out.extend_from_slice(&(wire.payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&wire.payload);
    }
}

fn append_number_wire(out: &mut Vec<u8>, n: &Number) -> Result<()> {
    let wire = NumericValueWire::encode(n).map_err(|_| unsupported_coeff_for_fingerprint())?;
    out.push(numeric_kind_tag(wire.kind));
    out.push(wire.sign);
    out.extend_from_slice(&(wire.payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&wire.payload);
    Ok(())
}

fn numeric_kind_tag(kind: NumericKind) -> u8 {
    match kind {
        NumericKind::Integer => 0,
        NumericKind::Rational => 1,
        NumericKind::Real => 2,
        NumericKind::Complex => 3,
        NumericKind::Interval => 4,
        NumericKind::Algebraic => 5,
        NumericKind::FiniteField => 6,
        NumericKind::Modular => 7,
        NumericKind::PAdic => 8,
    }
}

fn unsupported_coeff_for_fingerprint() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "fingerprint_coefficient_wire")
}

fn unknown_ring(ring: athena_types::RingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "fingerprint_unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
