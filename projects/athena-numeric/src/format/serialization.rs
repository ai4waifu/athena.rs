//! 数值序列化 wire（Integer / Rational / Real / Complex / Interval / Algebraic / FiniteField / Modular / PAdic · 冻结 binary `ANV1`）。

use athena_types::{Diagnostic, DiagnosticCode, NumericKind, SerializationVersion};

use crate::{
    number::NumericValue,
    precision::PrecisionInfo,
    wire_binary::{
        WireBlobParts, decode_algebraic_payload, decode_blob, decode_complex_payload, decode_finite_field_payload,
        decode_integer_payload, decode_interval_payload, decode_modular_payload, decode_padic_payload, decode_rational_payload,
        decode_real_payload, encode_algebraic_payload, encode_blob, encode_complex_payload, encode_finite_field_payload,
        encode_integer_payload, encode_interval_payload, encode_modular_payload, encode_padic_payload, encode_rational_payload,
        encode_real_payload,
    },
};

/// 跨进程 / arena 稳定数值载荷（`payload` 为 binary magnitude bytes，非十进制 UTF-8）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericValueWire {
    /// 种类。
    pub kind: NumericKind,
    /// 域描述字节（骨架；当前为空）。
    pub domain_payload: Vec<u8>,
    /// 值载荷（binary：`u32` limb count + little-endian `u64` limbs；有理数为 `numer||denom`）。
    pub payload: Vec<u8>,
    /// 符号（`0` 零 · `1` 正 · `2` 负；有理数指数分子符号）。
    pub sign: u8,
    /// 精度。
    pub precision: PrecisionInfo,
    /// schema 版本。
    pub version: SerializationVersion,
}

impl NumericValueWire {
    /// 当前 schema。
    pub fn current_version() -> SerializationVersion {
        SerializationVersion::CURRENT
    }

    /// 编码 [`NumericValue`]（Integer / Rational / Real / Complex / Interval / Algebraic / FiniteField / Modular / PAdic）。
    pub fn encode(value: &NumericValue) -> Result<Self, Diagnostic> {
        match value {
            NumericValue::Integer(n) => {
                let (sign, payload) = encode_integer_payload(n);
                Ok(Self {
                    kind: NumericKind::Integer,
                    domain_payload: Vec::new(),
                    payload,
                    sign,
                    precision: value.precision(),
                    version: Self::current_version(),
                })
            }
            NumericValue::Rational(r) => {
                let (sign, payload) = encode_rational_payload(r);
                Ok(Self {
                    kind: NumericKind::Rational,
                    domain_payload: Vec::new(),
                    payload,
                    sign,
                    precision: value.precision(),
                    version: Self::current_version(),
                })
            }
            NumericValue::Real(r) => {
                let (sign, payload) = encode_real_payload(r)?;
                Ok(Self {
                    kind: NumericKind::Real,
                    domain_payload: Vec::new(),
                    payload,
                    sign,
                    precision: value.precision(),
                    version: Self::current_version(),
                })
            }
            NumericValue::Complex(z) => {
                let (sign, payload) = encode_complex_payload(z)?;
                Ok(Self {
                    kind: NumericKind::Complex,
                    domain_payload: Vec::new(),
                    payload,
                    sign,
                    precision: value.precision(),
                    version: Self::current_version(),
                })
            }
            NumericValue::Interval(i) => {
                let (sign, payload) = encode_interval_payload(i)?;
                Ok(Self {
                    kind: NumericKind::Interval,
                    domain_payload: Vec::new(),
                    payload,
                    sign,
                    precision: value.precision(),
                    version: Self::current_version(),
                })
            }
            NumericValue::Modular(m) => {
                let (sign, payload) = encode_modular_payload(m)?;
                Ok(Self {
                    kind: NumericKind::Modular,
                    domain_payload: Vec::new(),
                    payload,
                    sign,
                    precision: value.precision(),
                    version: Self::current_version(),
                })
            }
            NumericValue::Algebraic(a) => {
                let (sign, payload) = encode_algebraic_payload(a)?;
                Ok(Self {
                    kind: NumericKind::Algebraic,
                    domain_payload: Vec::new(),
                    payload,
                    sign,
                    precision: value.precision(),
                    version: Self::current_version(),
                })
            }
            NumericValue::FiniteField(v) => {
                let (sign, payload) = encode_finite_field_payload(v)?;
                Ok(Self {
                    kind: NumericKind::FiniteField,
                    domain_payload: Vec::new(),
                    payload,
                    sign,
                    precision: value.precision(),
                    version: Self::current_version(),
                })
            }
            NumericValue::PAdic(v) => {
                let (sign, payload) = encode_padic_payload(v)?;
                Ok(Self {
                    kind: NumericKind::PAdic,
                    domain_payload: Vec::new(),
                    payload,
                    sign,
                    precision: value.precision(),
                    version: Self::current_version(),
                })
            }
        }
    }

    /// 解码为 [`NumericValue`]。
    pub fn decode(&self) -> Result<NumericValue, Diagnostic> {
        if self.version.0 > SerializationVersion::CURRENT.0 {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "numeric")
                .detail("operation", "serialize_version")
                .arg("version", u64::from(self.version.0)));
        }
        if self.payload.len() as u32 > crate::dispatch::PORTABLE_WIRE_PAYLOAD_LIMIT_BYTES {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_payload_limit"));
        }
        match self.kind {
            NumericKind::Integer => {
                let n = decode_integer_payload(self.sign, &self.payload)?;
                Ok(NumericValue::integer(n))
            }
            NumericKind::Rational => {
                let r = decode_rational_payload(self.sign, &self.payload)?;
                Ok(NumericValue::rational(r))
            }
            NumericKind::Real => {
                let r = decode_real_payload(self.sign, &self.payload)?;
                Ok(NumericValue::real(r))
            }
            NumericKind::Complex => {
                let z = decode_complex_payload(self.sign, &self.payload)?;
                Ok(NumericValue::complex(z))
            }
            NumericKind::Interval => {
                let i = decode_interval_payload(self.sign, &self.payload)?;
                Ok(NumericValue::interval(i))
            }
            NumericKind::Modular => {
                let m = decode_modular_payload(self.sign, &self.payload)?;
                Ok(NumericValue::modular(m))
            }
            NumericKind::Algebraic => {
                let a = decode_algebraic_payload(self.sign, &self.payload)?;
                Ok(NumericValue::algebraic(a))
            }
            NumericKind::FiniteField => {
                let v = decode_finite_field_payload(self.sign, &self.payload)?;
                Ok(NumericValue::finite_field(v))
            }
            NumericKind::PAdic => {
                let v = decode_padic_payload(self.sign, &self.payload)?;
                Ok(NumericValue::padic(v))
            }
        }
    }

    /// 展平为规范二进制 blob（`ANV1` 头 + 域 + 载荷）。
    pub fn to_bytes(&self) -> Result<Vec<u8>, Diagnostic> {
        encode_blob(self.version, self.kind, self.sign, &self.precision, &self.domain_payload, &self.payload)
    }

    /// 解析规范二进制 blob。
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Diagnostic> {
        let WireBlobParts { version, kind, sign, precision, domain_payload, payload } = decode_blob(bytes)?;
        Ok(Self { kind, domain_payload, payload, sign, precision, version })
    }
}
