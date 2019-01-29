//! 冻结二进制 wire 编解码（schema `ANV1`）。

use athena_types::{Diagnostic, DiagnosticCode, NumericKind, SerializationVersion};

use crate::{
    integer::Integer,
    natural::Natural,
    precision::{PrecisionInfo, PrecisionKind},
    rational::Rational,
};

/// 数值二进制 wire v1 的魔数。
pub const WIRE_MAGIC: &[u8; 4] = b"ANV1";

/// 固定头长度（字节）。
pub const WIRE_HEADER_LEN: usize = 28;

const FLAG_PRECISION_GUARANTEED: u8 = 1;

pub(crate) fn kind_to_tag(kind: NumericKind) -> u8 {
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

pub(crate) fn kind_from_tag(tag: u8) -> Option<NumericKind> {
    match tag {
        0 => Some(NumericKind::Integer),
        1 => Some(NumericKind::Rational),
        2 => Some(NumericKind::Real),
        3 => Some(NumericKind::Complex),
        4 => Some(NumericKind::Interval),
        5 => Some(NumericKind::Algebraic),
        6 => Some(NumericKind::FiniteField),
        7 => Some(NumericKind::Modular),
        8 => Some(NumericKind::PAdic),
        _ => None,
    }
}

fn precision_kind_to_tag(kind: PrecisionKind) -> u8 {
    match kind {
        PrecisionKind::Exact => 0,
        PrecisionKind::Machine => 1,
        PrecisionKind::Arbitrary => 2,
        PrecisionKind::Interval => 3,
        PrecisionKind::Certified => 4,
    }
}

fn precision_kind_from_tag(tag: u8) -> Option<PrecisionKind> {
    match tag {
        0 => Some(PrecisionKind::Exact),
        1 => Some(PrecisionKind::Machine),
        2 => Some(PrecisionKind::Arbitrary),
        3 => Some(PrecisionKind::Interval),
        4 => Some(PrecisionKind::Certified),
        _ => None,
    }
}

pub(crate) fn encode_precision(p: &PrecisionInfo) -> (u8, u8, u32, u32) {
    let mut flags = 0u8;
    if p.guaranteed {
        flags |= FLAG_PRECISION_GUARANTEED;
    }
    (precision_kind_to_tag(p.kind), flags, p.bits.unwrap_or(0), p.decimal_digits.unwrap_or(0))
}

pub(crate) fn decode_precision(kind_tag: u8, flags: u8, bits: u32, decimal: u32) -> Result<PrecisionInfo, Diagnostic> {
    let kind = precision_kind_from_tag(kind_tag).ok_or_else(|| wire_err("precision_kind"))?;
    Ok(PrecisionInfo {
        kind,
        bits: if bits == 0 { None } else { Some(bits) },
        decimal_digits: if decimal == 0 { None } else { Some(decimal) },
        guaranteed: flags & FLAG_PRECISION_GUARANTEED != 0,
    })
}

pub(crate) fn encode_integer_payload(n: &Integer) -> (u8, Vec<u8>) {
    (n.wire_sign_code(), n.wire_magnitude_bytes())
}

pub(crate) fn encode_rational_payload(r: &Rational) -> (u8, Vec<u8>) {
    let sign = r.numerator().wire_sign_code();
    let mut payload = r.numerator().wire_magnitude_bytes();
    payload.extend(r.denominator().wire_magnitude_bytes());
    (sign, payload)
}

pub(crate) fn decode_integer_payload(sign: u8, payload: &[u8]) -> Result<Integer, Diagnostic> {
    Integer::from_wire_magnitude(sign, payload)
}

pub(crate) fn decode_rational_payload(sign: u8, payload: &[u8]) -> Result<Rational, Diagnostic> {
    let (numer_mag, rest) = Natural::wire_take_magnitude(payload).map_err(|_| wire_err("rational_numer"))?;
    let (denom_mag, tail) = Natural::wire_take_magnitude(rest).map_err(|_| wire_err("rational_denom"))?;
    if !tail.is_empty() {
        return Err(wire_err("rational_trailing"));
    }
    let numer = Integer::from_wire_parts(sign, numer_mag)?;
    let denom = Integer::from_wire_parts(1, denom_mag)?;
    Rational::try_from_canonical_wire(numer, denom)
}

/// 将头 + 域 + 载荷展平为单一字节块。
pub fn encode_blob(
    version: SerializationVersion,
    kind: NumericKind,
    sign: u8,
    precision: &PrecisionInfo,
    domain_payload: &[u8],
    payload: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    check_payload_limit(domain_payload.len(), "domain")?;
    check_payload_limit(payload.len(), "payload")?;
    let (prec_kind, flags, bits, decimal) = encode_precision(precision);
    let mut out = Vec::with_capacity(WIRE_HEADER_LEN + domain_payload.len() + payload.len());
    out.extend_from_slice(WIRE_MAGIC);
    out.extend_from_slice(&version.0.to_le_bytes());
    out.push(kind_to_tag(kind));
    out.push(sign);
    out.push(prec_kind);
    out.push(flags);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(&decimal.to_le_bytes());
    out.extend_from_slice(&(domain_payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(domain_payload);
    out.extend_from_slice(payload);
    Ok(out)
}

/// 从平面 blob 解析出的 wire 各段。
pub struct WireBlobParts {
    /// schema 版本。
    pub version: SerializationVersion,
    /// 数值种类。
    pub kind: NumericKind,
    /// 符号码。
    pub sign: u8,
    /// 精度元数据。
    pub precision: PrecisionInfo,
    /// 域扩展字节。
    pub domain_payload: Vec<u8>,
    /// 值载荷字节。
    pub payload: Vec<u8>,
}

/// 将平面二进制 blob 解析为 wire 各段。
pub fn decode_blob(bytes: &[u8]) -> Result<WireBlobParts, Diagnostic> {
    if bytes.len() < WIRE_HEADER_LEN {
        return Err(wire_err("header_short"));
    }
    if bytes.get(0..4) != Some(WIRE_MAGIC) {
        return Err(wire_err("magic"));
    }
    let version = SerializationVersion(u16::from_le_bytes(bytes[4..6].try_into().unwrap()));
    if version.0 > SerializationVersion::CURRENT.0 {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "numeric")
            .detail("operation", "wire_version")
            .arg("version", u64::from(version.0)));
    }
    let kind = kind_from_tag(bytes[6]).ok_or_else(|| wire_err("kind"))?;
    let sign = bytes[7];
    let precision = decode_precision(
        bytes[8],
        bytes[9],
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
    )?;
    let domain_len = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
    let payload_len = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
    check_payload_limit(domain_len, "domain")?;
    check_payload_limit(payload_len, "payload")?;
    let body_start = WIRE_HEADER_LEN;
    let domain_end = body_start.checked_add(domain_len).ok_or_else(|| wire_err("domain_len"))?;
    let payload_end = domain_end.checked_add(payload_len).ok_or_else(|| wire_err("payload_len"))?;
    if bytes.len() != payload_end {
        return Err(wire_err("length"));
    }
    Ok(WireBlobParts {
        version,
        kind,
        sign,
        precision,
        domain_payload: bytes[body_start..domain_end].to_vec(),
        payload: bytes[domain_end..payload_end].to_vec(),
    })
}

fn check_payload_limit(len: usize, op: &str) -> Result<(), Diagnostic> {
    if len as u32 > crate::backend::PURE_RUST_WIRE_PAYLOAD_LIMIT_BYTES {
        return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
            .detail("domain", "numeric")
            .detail("operation", op));
    }
    Ok(())
}

fn wire_err(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", op)
}
