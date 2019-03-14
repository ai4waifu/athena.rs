//! 冻结二进制 wire 编解码（schema `ANV1`）。

use athena_types::{Diagnostic, DiagnosticCode, NumericKind, SerializationVersion};

use crate::{
    algebraic::{AlgebraicNumber, AlgebraicRepresentation},
    complex::{BranchPolicy, Complex},
    decimal::Decimal,
    dyadic::Dyadic,
    finite_field::FiniteFieldValue,
    integer::{Integer, Sign},
    interval::{Interval, IntervalDecoration},
    modular::{ModularValue, Modulus},
    natural::Natural,
    p_adic::PAdicValue,
    polynomial_fingerprint::PolynomialFingerprint,
    precision::{PrecisionInfo, PrecisionKind},
    rational::Rational,
    real::Real,
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
    use crate::format::validation::{WireReject, reject_non_canonical};
    let (numer_mag, rest) = Natural::wire_take_magnitude(payload)?;
    let (denom_mag, tail) = Natural::wire_take_magnitude(rest)?;
    if !tail.is_empty() {
        return Err(reject_non_canonical(WireReject::RationalTrailing));
    }
    if denom_mag.is_zero() {
        return Err(reject_non_canonical(WireReject::RationalDenomZero));
    }
    let numer = Integer::from_wire_parts(sign, numer_mag)?;
    let denom = Integer::from_wire_parts(1, denom_mag)?;
    Rational::try_from_canonical_wire(numer, denom)
}

const REAL_SUBTYPE_MACHINE: u8 = 0;
const REAL_SUBTYPE_DECIMAL: u8 = 1;

/// 编码 [`Real`]：`subtype` + 载荷；header `sign` 对 Machine 为 0，对 Decimal 为尾数符号。
pub(crate) fn encode_real_payload(r: &Real) -> Result<(u8, Vec<u8>), Diagnostic> {
    match r {
        Real::Machine(x) => {
            if x.is_nan() {
                return Err(crate::format::validation::reject_non_canonical(
                    crate::format::validation::WireReject::RealMachineNan,
                ));
            }
            let mut payload = Vec::with_capacity(1 + 8);
            payload.push(REAL_SUBTYPE_MACHINE);
            payload.extend_from_slice(&x.to_bits().to_le_bytes());
            Ok((0, payload))
        }
        Real::Decimal(d) => {
            d.validate().map_err(|_| {
                crate::format::validation::reject_non_canonical(crate::format::validation::WireReject::RealDecimalNotNormalized)
            })?;
            let sign = match d.sign() {
                Sign::Zero => 0u8,
                Sign::Positive => 1,
                Sign::Negative => 2,
            };
            let mut payload = Vec::with_capacity(1 + 4 + 8 + 8 + 4);
            payload.push(REAL_SUBTYPE_DECIMAL);
            payload.extend_from_slice(&d.significand().wire_encode_magnitude());
            payload.extend_from_slice(&d.exponent().to_le_bytes());
            payload.extend_from_slice(&d.precision_bits().to_le_bytes());
            Ok((sign, payload))
        }
    }
}

/// 解码 Real 载荷（拒绝 NaN / 非法 subtype / 非规范 Decimal）。
pub(crate) fn decode_real_payload(sign: u8, payload: &[u8]) -> Result<Real, Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    if payload.is_empty() {
        return Err(reject_non_canonical(WireReject::RealUnknownSubtype));
    }
    match payload[0] {
        REAL_SUBTYPE_MACHINE => {
            if payload.len() != 1 + 8 {
                return Err(reject_non_canonical(WireReject::RealMachineLen));
            }
            if sign != 0 {
                return Err(reject_non_canonical(WireReject::SignUnknown));
            }
            let bits = u64::from_le_bytes(payload[1..9].try_into().unwrap());
            let x = f64::from_bits(bits);
            if x.is_nan() {
                return Err(reject_non_canonical(WireReject::RealMachineNan));
            }
            Ok(Real::Machine(x))
        }
        REAL_SUBTYPE_DECIMAL => {
            let (mag, rest) = Natural::wire_take_magnitude(&payload[1..])?;
            if rest.len() < 12 {
                return Err(reject_non_canonical(WireReject::RealDecimalTrailing));
            }
            let exp = i64::from_le_bytes(rest[..8].try_into().unwrap());
            let precision_bits = u32::from_le_bytes(rest[8..12].try_into().unwrap());
            if !rest[12..].is_empty() {
                return Err(reject_non_canonical(WireReject::RealDecimalTrailing));
            }
            if precision_bits < crate::decimal::MIN_PRECISION_BITS {
                return Err(reject_non_canonical(WireReject::RealDecimalPrecisionZero));
            }
            if mag.is_zero() {
                if exp != 0 {
                    return Err(reject_non_canonical(WireReject::RealDecimalZeroExp));
                }
                // IEEE `-0`：允许 sign=2 + 零幅度。
                if sign == 1 {
                    return Err(reject_non_canonical(WireReject::SignPosZeroMag));
                }
                if sign > 2 {
                    return Err(reject_non_canonical(WireReject::SignUnknown));
                }
                let sign = if sign == 2 { Sign::Negative } else { Sign::Zero };
                let dyadic = Dyadic::try_new(sign, Natural::zero(), 0)
                    .map_err(|_| reject_non_canonical(WireReject::RealDecimalNotNormalized))?;
                return Decimal::try_from_dyadic(dyadic, precision_bits)
                    .map(Real::Decimal)
                    .map_err(|_| reject_non_canonical(WireReject::RealDecimalPrecisionExceeds));
            }
            if sign != 1 && sign != 2 {
                return Err(reject_non_canonical(WireReject::SignUnknown));
            }
            if !mag.is_odd() {
                return Err(reject_non_canonical(WireReject::RealDecimalNotNormalized));
            }
            let bits = mag.bits();
            if bits > u64::from(precision_bits) {
                return Err(reject_non_canonical(WireReject::RealDecimalPrecisionExceeds));
            }
            let sign = if sign == 2 { Sign::Negative } else { Sign::Positive };
            let dyadic =
                Dyadic::try_new(sign, mag, exp).map_err(|_| reject_non_canonical(WireReject::RealDecimalNotNormalized))?;
            Decimal::try_from_dyadic(dyadic, precision_bits)
                .map(Real::Decimal)
                .map_err(|_| reject_non_canonical(WireReject::RealDecimalPrecisionExceeds))
        }
        _ => Err(reject_non_canonical(WireReject::RealUnknownSubtype)),
    }
}

const INTERVAL_EMPTY: u8 = 0;
const INTERVAL_ENTIRE: u8 = 1;
const INTERVAL_BOUNDED: u8 = 2;

fn decoration_to_tag(d: IntervalDecoration) -> u8 {
    match d {
        IntervalDecoration::Certain => 0,
        IntervalDecoration::Defined => 1,
        IntervalDecoration::Trivial => 2,
        IntervalDecoration::Ill => 3,
    }
}

fn decoration_from_tag(tag: u8) -> Option<IntervalDecoration> {
    match tag {
        0 => Some(IntervalDecoration::Certain),
        1 => Some(IntervalDecoration::Defined),
        2 => Some(IntervalDecoration::Trivial),
        3 => Some(IntervalDecoration::Ill),
        _ => None,
    }
}

fn encode_nested_real(r: &Real) -> Result<Vec<u8>, Diagnostic> {
    let (sign, payload) = encode_real_payload(r)?;
    let mut out = Vec::with_capacity(1 + payload.len());
    out.push(sign);
    out.extend_from_slice(&payload);
    Ok(out)
}

fn decode_nested_real(bytes: &[u8]) -> Result<(Real, &[u8]), Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    if bytes.is_empty() {
        return Err(reject_non_canonical(WireReject::IntervalTrailing));
    }
    let sign = bytes[0];
    let rest = &bytes[1..];
    // Real payload is self-describing by subtype length for Machine; Decimal needs take_magnitude.
    if rest.is_empty() {
        return Err(reject_non_canonical(WireReject::IntervalTrailing));
    }
    match rest[0] {
        REAL_SUBTYPE_MACHINE => {
            if rest.len() < 1 + 8 {
                return Err(reject_non_canonical(WireReject::IntervalTrailing));
            }
            let r = decode_real_payload(sign, &rest[..1 + 8])?;
            Ok((r, &rest[1 + 8..]))
        }
        REAL_SUBTYPE_DECIMAL => {
            let (_mag, after_mag) = Natural::wire_take_magnitude(&rest[1..])?;
            if after_mag.len() < 12 {
                return Err(reject_non_canonical(WireReject::IntervalTrailing));
            }
            let mag_bytes = rest[1..].len() - after_mag.len();
            let used = 1 + mag_bytes + 12;
            let chunk = &rest[..used];
            let r = decode_real_payload(sign, chunk)?;
            Ok((r, &rest[used..]))
        }
        _ => Err(reject_non_canonical(WireReject::RealUnknownSubtype)),
    }
}

/// 编码 [`Interval`]（header `sign` 恒 0）。
pub(crate) fn encode_interval_payload(i: &Interval) -> Result<(u8, Vec<u8>), Diagnostic> {
    match i {
        Interval::Empty => Ok((0, vec![INTERVAL_EMPTY])),
        Interval::Entire { decoration } => Ok((0, vec![INTERVAL_ENTIRE, decoration_to_tag(*decoration)])),
        Interval::Bounded { lower, upper, decoration } => {
            let mut payload = vec![INTERVAL_BOUNDED, decoration_to_tag(*decoration)];
            payload.extend(encode_nested_real(lower)?);
            payload.extend(encode_nested_real(upper)?);
            Ok((0, payload))
        }
    }
}

/// 解码 Interval 载荷。
pub(crate) fn decode_interval_payload(sign: u8, payload: &[u8]) -> Result<Interval, Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    if sign != 0 {
        return Err(reject_non_canonical(WireReject::SignUnknown));
    }
    if payload.is_empty() {
        return Err(reject_non_canonical(WireReject::IntervalUnknownSubtype));
    }
    match payload[0] {
        INTERVAL_EMPTY => {
            if payload.len() != 1 {
                return Err(reject_non_canonical(WireReject::IntervalTrailing));
            }
            Ok(Interval::empty())
        }
        INTERVAL_ENTIRE => {
            if payload.len() != 2 {
                return Err(reject_non_canonical(WireReject::IntervalTrailing));
            }
            let decoration =
                decoration_from_tag(payload[1]).ok_or_else(|| reject_non_canonical(WireReject::IntervalUnknownDecoration))?;
            Ok(Interval::entire_with(decoration))
        }
        INTERVAL_BOUNDED => {
            if payload.len() < 2 {
                return Err(reject_non_canonical(WireReject::IntervalTrailing));
            }
            let decoration =
                decoration_from_tag(payload[1]).ok_or_else(|| reject_non_canonical(WireReject::IntervalUnknownDecoration))?;
            let (lower, rest) = decode_nested_real(&payload[2..])?;
            let (upper, tail) = decode_nested_real(rest)?;
            if !tail.is_empty() {
                return Err(reject_non_canonical(WireReject::IntervalTrailing));
            }
            Interval::try_bounded(lower, upper, decoration).map_err(|_| reject_non_canonical(WireReject::IntervalBadBounds))
        }
        _ => Err(reject_non_canonical(WireReject::IntervalUnknownSubtype)),
    }
}

/// 编码嵌入模数的 [`ModularValue`]（header `sign` = 剩余符号）。
pub(crate) fn encode_modular_payload(v: &ModularValue) -> Result<(u8, Vec<u8>), Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    let modulus = v.modulus().ok_or_else(|| reject_non_canonical(WireReject::ModularInterned))?;
    let residue = v.residue();
    let (sign, mut payload) = encode_integer_payload(&residue);
    payload.extend(modulus.value().wire_magnitude_bytes());
    Ok((sign, payload))
}

/// 解码 Modular 载荷（仅嵌入模数）。
pub(crate) fn decode_modular_payload(sign: u8, payload: &[u8]) -> Result<ModularValue, Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    let (res_mag, rest) = Natural::wire_take_magnitude(payload)?;
    let (mod_mag, tail) = Natural::wire_take_magnitude(rest)?;
    if !tail.is_empty() {
        return Err(reject_non_canonical(WireReject::ModularTrailing));
    }
    let residue = Integer::from_wire_parts(sign, res_mag)?;
    if mod_mag.is_zero() || mod_mag.is_one() {
        return Err(reject_non_canonical(WireReject::ModularBadModulus));
    }
    let mod_int = Integer::from_wire_parts(1, mod_mag)?;
    let modulus = Modulus::new(mod_int.clone()).map_err(|_| reject_non_canonical(WireReject::ModularBadModulus))?;
    if residue.is_negative() || residue >= mod_int {
        return Err(reject_non_canonical(WireReject::ModularResidueUnreduced));
    }
    Ok(ModularValue::new(residue, modulus))
}

fn branch_to_tag(branch: BranchPolicy) -> u8 {
    match branch {
        BranchPolicy::Principal => 0,
        BranchPolicy::RealOnly => 1,
    }
}

fn branch_from_tag(tag: u8) -> Option<BranchPolicy> {
    match tag {
        0 => Some(BranchPolicy::Principal),
        1 => Some(BranchPolicy::RealOnly),
        _ => None,
    }
}

fn map_nested_real_truncation(err: Diagnostic) -> Diagnostic {
    use crate::format::validation::{WireReject, reject_non_canonical};
    use athena_types::DiagnosticValue;
    match err.details.get("reason") {
        Some(DiagnosticValue::Text(s)) if s == "interval_trailing" => reject_non_canonical(WireReject::ComplexTrailing),
        _ => err,
    }
}

/// 编码 [`Complex`]（header `sign` 恒 0；载荷 = branch + nested re + nested im）。
pub(crate) fn encode_complex_payload(z: &Complex) -> Result<(u8, Vec<u8>), Diagnostic> {
    let mut payload = vec![branch_to_tag(z.branch)];
    payload.extend(encode_nested_real(&z.re)?);
    payload.extend(encode_nested_real(&z.im)?);
    Ok((0, payload))
}

/// 解码 Complex 载荷。
pub(crate) fn decode_complex_payload(sign: u8, payload: &[u8]) -> Result<Complex, Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    if sign != 0 {
        return Err(reject_non_canonical(WireReject::SignUnknown));
    }
    if payload.is_empty() {
        return Err(reject_non_canonical(WireReject::ComplexTrailing));
    }
    let branch = branch_from_tag(payload[0]).ok_or_else(|| reject_non_canonical(WireReject::ComplexUnknownBranch))?;
    let (re, rest) = decode_nested_real(&payload[1..]).map_err(map_nested_real_truncation)?;
    let (im, tail) = decode_nested_real(rest).map_err(map_nested_real_truncation)?;
    if !tail.is_empty() {
        return Err(reject_non_canonical(WireReject::ComplexTrailing));
    }
    Complex::try_new(re, im, branch).map_err(|_| reject_non_canonical(WireReject::RealMachineNan))
}

const ALG_PLACEHOLDER: u8 = 0;
const ALG_MINPOLY: u8 = 1;

/// 编码 [`AlgebraicNumber`]（header `sign` 恒 0）。
///
/// 载荷：`tag` + 指纹 `u64` + `root_index` `u32` + interval payload。
pub(crate) fn encode_algebraic_payload(a: &AlgebraicNumber) -> Result<(u8, Vec<u8>), Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    a.validate().map_err(|_| reject_non_canonical(WireReject::AlgebraicInconsistent))?;
    let (tag, fingerprint, root_index) = match &a.representation {
        AlgebraicRepresentation::Placeholder => (ALG_PLACEHOLDER, 0u64, 0u32),
        AlgebraicRepresentation::MinimalPolynomial { polynomial, root_index } => (ALG_MINPOLY, polynomial.0, *root_index),
    };
    let mut payload = Vec::with_capacity(1 + 8 + 4);
    payload.push(tag);
    payload.extend_from_slice(&fingerprint.to_le_bytes());
    payload.extend_from_slice(&root_index.to_le_bytes());
    let (_, interval_payload) = encode_interval_payload(&a.isolating_interval)?;
    payload.extend(interval_payload);
    Ok((0, payload))
}

/// 解码 Algebraic 载荷。
pub(crate) fn decode_algebraic_payload(sign: u8, payload: &[u8]) -> Result<AlgebraicNumber, Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    if sign != 0 {
        return Err(reject_non_canonical(WireReject::SignUnknown));
    }
    if payload.len() < 1 + 8 + 4 {
        return Err(reject_non_canonical(WireReject::AlgebraicTrailing));
    }
    let tag = payload[0];
    let fingerprint = u64::from_le_bytes(payload[1..9].try_into().unwrap());
    let root_index = u32::from_le_bytes(payload[9..13].try_into().unwrap());
    let interval = decode_interval_payload(0, &payload[13..])
        .map_err(|err| map_interval_truncation(err, WireReject::AlgebraicTrailing))?;
    match tag {
        ALG_PLACEHOLDER => {
            if fingerprint != 0 || root_index != 0 {
                return Err(reject_non_canonical(WireReject::AlgebraicPlaceholder));
            }
            AlgebraicNumber::placeholder(interval).map_err(|_| reject_non_canonical(WireReject::AlgebraicInconsistent))
        }
        ALG_MINPOLY => AlgebraicNumber::try_new(
            PolynomialFingerprint(fingerprint),
            interval,
            AlgebraicRepresentation::MinimalPolynomial { polynomial: PolynomialFingerprint(fingerprint), root_index },
        )
        .map_err(|_| reject_non_canonical(WireReject::AlgebraicInconsistent)),
        _ => Err(reject_non_canonical(WireReject::AlgebraicUnknownSubtype)),
    }
}

fn map_interval_truncation(err: Diagnostic, trailing: crate::format::validation::WireReject) -> Diagnostic {
    use crate::format::validation::reject_non_canonical;
    use athena_types::DiagnosticValue;
    match err.details.get("reason") {
        Some(DiagnosticValue::Text(s)) if s == "interval_trailing" => reject_non_canonical(trailing),
        _ => err,
    }
}

fn encode_signed_integer(n: &Integer) -> Vec<u8> {
    let mut out = vec![n.wire_sign_code()];
    out.extend(n.wire_magnitude_bytes());
    out
}

fn take_signed_integer(bytes: &[u8]) -> Result<(Integer, &[u8]), Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    if bytes.is_empty() {
        return Err(reject_non_canonical(WireReject::FiniteFieldTrailing));
    }
    let sign = bytes[0];
    let (mag, rest) = Natural::wire_take_magnitude(&bytes[1..])?;
    Ok((Integer::from_wire_parts(sign, mag)?, rest))
}

/// 编码 [`FiniteFieldValue`]（header `sign` 恒 0）。
///
/// 载荷：`FieldId` `u32` + `FieldPresentationId` `u32` + 系数个数 `u32` + 逐项 `(sign, magnitude)`。
/// 二者均为 Session-local 句柄。
pub(crate) fn encode_finite_field_payload(v: &FiniteFieldValue) -> Result<(u8, Vec<u8>), Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    v.validate().map_err(|_| reject_non_canonical(WireReject::FiniteFieldEmpty))?;
    let mut payload = Vec::new();
    payload.extend_from_slice(&v.field().0.to_le_bytes());
    payload.extend_from_slice(&v.presentation().0.to_le_bytes());
    payload.extend_from_slice(&(v.coefficients().len() as u32).to_le_bytes());
    for c in v.coefficients() {
        payload.extend(encode_signed_integer(c));
    }
    Ok((0, payload))
}

/// 解码 FiniteField 载荷。
pub(crate) fn decode_finite_field_payload(sign: u8, payload: &[u8]) -> Result<FiniteFieldValue, Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    use athena_types::{FieldId, FieldPresentationId};
    if sign != 0 {
        return Err(reject_non_canonical(WireReject::SignUnknown));
    }
    if payload.len() < 12 {
        return Err(reject_non_canonical(WireReject::FiniteFieldTrailing));
    }
    let field = FieldId(u32::from_le_bytes(payload[0..4].try_into().unwrap()));
    let presentation = FieldPresentationId(u32::from_le_bytes(payload[4..8].try_into().unwrap()));
    let count = u32::from_le_bytes(payload[8..12].try_into().unwrap()) as usize;
    if count == 0 {
        return Err(reject_non_canonical(WireReject::FiniteFieldEmpty));
    }
    let mut rest = &payload[12..];
    let mut coefficients = Vec::with_capacity(count);
    for _ in 0..count {
        let (c, tail) = take_signed_integer(rest)?;
        coefficients.push(c);
        rest = tail;
    }
    if !rest.is_empty() {
        return Err(reject_non_canonical(WireReject::FiniteFieldTrailing));
    }
    FiniteFieldValue::try_new(field, presentation, coefficients)
        .map_err(|_| reject_non_canonical(WireReject::FiniteFieldEmpty))
}

/// 编码 [`PAdicValue`]（header `sign` 恒 0）。
///
/// 载荷：prime magnitude + `precision` `u32` + digit 个数 `u32` + 小端 `u32` digits。
pub(crate) fn encode_padic_payload(v: &PAdicValue) -> Result<(u8, Vec<u8>), Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    v.validate().map_err(|err| map_padic_validate(err))?;
    if v.digits.last() == Some(&0) {
        return Err(reject_non_canonical(WireReject::PAdicUnnormalized));
    }
    let mut payload = v.prime.wire_magnitude_bytes();
    payload.extend_from_slice(&v.precision.to_le_bytes());
    payload.extend_from_slice(&(v.digits.len() as u32).to_le_bytes());
    for &d in &v.digits {
        payload.extend_from_slice(&d.to_le_bytes());
    }
    Ok((0, payload))
}

/// 解码 PAdic 载荷（拒绝未规范化 trailing-zero digits）。
pub(crate) fn decode_padic_payload(sign: u8, payload: &[u8]) -> Result<PAdicValue, Diagnostic> {
    use crate::format::validation::{WireReject, reject_non_canonical};
    if sign != 0 {
        return Err(reject_non_canonical(WireReject::SignUnknown));
    }
    let (prime_mag, rest) = Natural::wire_take_magnitude(payload)?;
    if rest.len() < 8 {
        return Err(reject_non_canonical(WireReject::PAdicTrailing));
    }
    if prime_mag.is_zero() {
        return Err(reject_non_canonical(WireReject::PAdicBadPrime));
    }
    let prime = Integer::from_wire_parts(1, prime_mag)?;
    let precision = u32::from_le_bytes(rest[0..4].try_into().unwrap());
    let digit_count = u32::from_le_bytes(rest[4..8].try_into().unwrap()) as usize;
    let digit_bytes = &rest[8..];
    let need = digit_count.checked_mul(4).ok_or_else(|| reject_non_canonical(WireReject::PAdicTrailing))?;
    if digit_bytes.len() != need {
        return Err(reject_non_canonical(WireReject::PAdicTrailing));
    }
    let mut digits = Vec::with_capacity(digit_count);
    for i in 0..digit_count {
        let off = i * 4;
        digits.push(u32::from_le_bytes(digit_bytes[off..off + 4].try_into().unwrap()));
    }
    if digits.last() == Some(&0) {
        return Err(reject_non_canonical(WireReject::PAdicUnnormalized));
    }
    PAdicValue::try_new(prime, precision, digits).map_err(map_padic_validate)
}

fn map_padic_validate(err: Diagnostic) -> Diagnostic {
    use crate::format::validation::{WireReject, reject_non_canonical};
    use athena_types::DiagnosticValue;
    match err.details.get("operation") {
        Some(DiagnosticValue::Text(s)) if s == "padic_precision_zero" => reject_non_canonical(WireReject::PAdicPrecisionZero),
        Some(DiagnosticValue::Text(s)) if s == "padic_digits_len" => reject_non_canonical(WireReject::PAdicDigitsLen),
        Some(DiagnosticValue::Text(s)) if s == "padic_digit_range" => reject_non_canonical(WireReject::PAdicDigitRange),
        Some(DiagnosticValue::Text(s)) if s.starts_with("padic_prime") => reject_non_canonical(WireReject::PAdicBadPrime),
        _ => err,
    }
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
    if len as u32 > crate::dispatch::PORTABLE_WIRE_PAYLOAD_LIMIT_BYTES {
        return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
            .detail("domain", "numeric")
            .detail("operation", op));
    }
    Ok(())
}

fn wire_err(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", op)
}
