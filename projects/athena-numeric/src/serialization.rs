//! 数值序列化 wire（N1：Integer / Rational）。

use athena_types::{Diagnostic, DiagnosticCode, NumericKind, SerializationVersion};
use std::str::FromStr;

use crate::{
    integer::Integer,
    number::{NumericRepr, NumericValue},
    precision::PrecisionInfo,
    rational::Rational,
};

/// 跨进程 / arena 稳定数值载荷。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericValueWire {
    /// 种类。
    pub kind: NumericKind,
    /// 域描述字节（骨架；当前为空）。
    pub domain_payload: Vec<u8>,
    /// 值载荷（UTF-8：整数十进制，或 `numer/denom`）。
    pub payload: Vec<u8>,
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

    /// 编码 [`NumericValue`]（N1 覆盖 Integer / Rational）。
    pub fn encode(value: &NumericValue) -> Result<Self, Diagnostic> {
        match value.repr() {
            NumericRepr::Integer(n) => Ok(Self {
                kind: NumericKind::Integer,
                domain_payload: Vec::new(),
                payload: n.to_decimal_string().into_bytes(),
                precision: value.precision().clone(),
                version: Self::current_version(),
            }),
            NumericRepr::Rational(r) => Ok(Self {
                kind: NumericKind::Rational,
                domain_payload: Vec::new(),
                payload: r.to_wire_string().into_bytes(),
                precision: value.precision().clone(),
                version: Self::current_version(),
            }),
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "numeric")
                .detail("operation", "serialize")),
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
        let text = std::str::from_utf8(&self.payload).map_err(|_| {
            Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "deserialize_utf8")
        })?;
        if self.payload.len() as u32 > crate::backend::PURE_RUST_WIRE_PAYLOAD_LIMIT_BYTES {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "wire_payload_limit"));
        }
        match self.kind {
            NumericKind::Integer => {
                let n = Integer::from_str(text).map_err(|_| {
                    Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                        .detail("domain", "numeric")
                        .detail("operation", "deserialize_integer")
                })?;
                Ok(NumericValue::integer(n))
            }
            NumericKind::Rational => {
                let r = Rational::decode_wire_payload(text)?;
                Ok(NumericValue::rational(r))
            }
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "numeric")
                .detail("operation", "deserialize_kind")),
        }
    }
}
